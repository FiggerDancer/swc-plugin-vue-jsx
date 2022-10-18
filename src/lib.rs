use std::collections::HashMap;
use swc_core::{
    common::DUMMY_SP,
    ecma::{
        ast::*,
        transforms::testing::test,
        utils::{private_ident, quote_ident, quote_str},
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
    plugin::{plugin_transform, proxies::TransformPluginProgramMetadata},
};

pub struct VueJsxTransformVisitor {
    imports: HashMap<&'static str, Ident>,
}

impl VueJsxTransformVisitor {
    fn transform_tag(&mut self, jsx_element_name: &JSXElementName) -> Expr {
        match jsx_element_name {
            JSXElementName::Ident(ident) => Expr::Ident(ident.clone()),
            JSXElementName::JSXMemberExpr(expr) => Expr::JSXMember(expr.clone()),
            JSXElementName::JSXNamespacedName(name) => Expr::JSXNamespacedName(name.clone()),
        }
    }

    fn transform_attrs(&mut self, attrs: &[JSXAttrOrSpread]) -> ObjectLit {
        ObjectLit {
            span: DUMMY_SP,
            props: attrs
                .iter()
                .map(|jsx_attr_or_spread| match jsx_attr_or_spread {
                    JSXAttrOrSpread::JSXAttr(jsx_attr) => {
                        PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                            key: match &jsx_attr.name {
                                JSXAttrName::Ident(ident) => PropName::Str(quote_str!(&ident.sym)),
                                JSXAttrName::JSXNamespacedName(name) => PropName::Str(quote_str!(
                                    format!("{}:{}", name.ns.sym, name.name.sym)
                                )),
                            },
                            value: jsx_attr
                                .value
                                .as_ref()
                                .map(|value| match value {
                                    JSXAttrValue::Lit(lit) => Box::new(Expr::Lit(lit.clone())),
                                    JSXAttrValue::JSXExprContainer(JSXExprContainer {
                                        expr: JSXExpr::Expr(expr),
                                        ..
                                    }) => expr.clone(),
                                    JSXAttrValue::JSXExprContainer(JSXExprContainer {
                                        expr: JSXExpr::JSXEmptyExpr(expr),
                                        ..
                                    }) => Box::new(Expr::JSXEmpty(expr.clone())),
                                    JSXAttrValue::JSXElement(element) => {
                                        Box::new(Expr::JSXElement(element.clone()))
                                    }
                                    JSXAttrValue::JSXFragment(fragment) => {
                                        Box::new(Expr::JSXFragment(fragment.clone()))
                                    }
                                })
                                .unwrap_or_else(|| {
                                    Box::new(Expr::Lit(Lit::Bool(Bool {
                                        span: DUMMY_SP,
                                        value: true,
                                    })))
                                }),
                        })))
                    }
                    JSXAttrOrSpread::SpreadElement(spread) => PropOrSpread::Spread(spread.clone()),
                })
                .collect(),
        }
    }
}

impl VisitMut for VueJsxTransformVisitor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        module.visit_mut_children_with(self);

        module.body.insert(
            0,
            ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                span: DUMMY_SP,
                specifiers: self
                    .imports
                    .iter()
                    .map(|(imported, local)| {
                        ImportSpecifier::Named(ImportNamedSpecifier {
                            span: DUMMY_SP,
                            local: local.clone(),
                            imported: Some(ModuleExportName::Ident(quote_ident!(*imported))),
                            is_type_only: false,
                        })
                    })
                    .collect(),
                src: Box::new(quote_str!("vue")),
                type_only: false,
                asserts: None,
            })),
        )
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::JSXElement(jsx) = expr {
            *expr = Expr::Call(CallExpr {
                span: DUMMY_SP,
                callee: Callee::Expr(Box::new(Expr::Ident(
                    self.imports
                        .entry("createVNode")
                        .or_insert_with_key(|name| private_ident!(*name))
                        .clone(),
                ))),
                args: vec![
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(self.transform_tag(&jsx.opening.name)),
                    },
                    ExprOrSpread {
                        spread: None,
                        expr: Box::new(if jsx.opening.attrs.is_empty() {
                            Expr::Lit(Lit::Null(Null { span: DUMMY_SP }))
                        } else {
                            Expr::Object(self.transform_attrs(&jsx.opening.attrs))
                        }),
                    },
                ],
                type_args: None,
            });
        }

        expr.visit_mut_children_with(self);
    }
}

#[plugin_transform]
pub fn vue_jsx(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(VueJsxTransformVisitor {
        imports: Default::default(),
    }))
}

test!(
    swc_ecma_parser::Syntax::Es(swc_ecma_parser::EsConfig {
        jsx: true,
        ..Default::default()
    }),
    |_| as_folder(VueJsxTransformVisitor {
        imports: Default::default(),
    }),
    basic,
    r#"const App = <Comp v={afa}></Comp>;"#,
    r#""#
);
