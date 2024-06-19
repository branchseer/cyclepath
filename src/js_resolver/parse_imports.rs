use oxc_allocator::Allocator;
use oxc_ast::{
    ast::{Argument, Expression, ImportOrExportKind},
    visit::{
        walk::{
            walk_call_expression, walk_export_all_declaration, walk_export_named_declaration,
            walk_import_declaration, walk_import_expression, walk_program,
            walk_ts_import_equals_declaration,
        },
        Visit,
    },
};
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::Parser;
use oxc_span::GetSpan;
use oxc_span::{SourceType, Span};

#[derive(Default)]
pub struct Imports<'a> {
    pub specifiers: Vec<(&'a str, Span)>,
    pub non_literal_imports: Vec<Span>,
}

pub fn parse_imports<'a>(
    allocator: &'a Allocator,
    source_type: SourceType,
    source: &'a str,
) -> (Imports<'a>, Vec<OxcDiagnostic>) {
    let parser = Parser::new(allocator, source, source_type);
    let parse_return = parser.parse();
    if parse_return.panicked {
        return (Default::default(), parse_return.errors);
    }

    #[derive(Default)]
    struct ImportsVisitor<'a> {
        specifiers: Vec<(&'a str, Span)>,
        non_literal_imports: Vec<Span>,
    }
    impl<'a> Visit<'a> for ImportsVisitor<'a> {
        fn visit_export_all_declaration(&mut self, decl: &oxc_ast::ast::ExportAllDeclaration<'a>) {
            if !decl.export_kind.is_type() {
                self.specifiers
                    .push((decl.source.value.as_str(), decl.source.span));
            }
            walk_export_all_declaration(self, decl);
        }
        fn visit_export_named_declaration(
            &mut self,
            decl: &oxc_ast::ast::ExportNamedDeclaration<'a>,
        ) {
            if !decl.export_kind.is_type() {
                if let Some(source) = &decl.source {
                    self.specifiers.push((source.value.as_str(), source.span))
                }
            }
            walk_export_named_declaration(self, decl);
        }
        fn visit_import_declaration(&mut self, decl: &oxc_ast::ast::ImportDeclaration<'a>) {
            if !decl.import_kind.is_type() {
                self.specifiers
                    .push((decl.source.value.as_str(), decl.source.span))
            };
            walk_import_declaration(self, decl)
        }
        fn visit_ts_import_equals_declaration(
            &mut self,
            decl: &oxc_ast::ast::TSImportEqualsDeclaration<'a>,
        ) {
            if !decl.import_kind.is_type() {
                if let oxc_ast::ast::TSModuleReference::ExternalModuleReference(
                    external_module_reference,
                ) = &decl.module_reference
                {
                    let specifier_literal = &external_module_reference.expression;
                    self.specifiers
                        .push((specifier_literal.value.as_str(), specifier_literal.span))
                }
            };
            walk_ts_import_equals_declaration(self, decl)
        }
        fn visit_import_expression(&mut self, expr: &oxc_ast::ast::ImportExpression<'a>) {
            if let Expression::StringLiteral(string_literal) = &expr.source {
                self.specifiers
                    .push((string_literal.value.as_str(), string_literal.span))
            } else {
                self.non_literal_imports.push(expr.source.span())
            }
            walk_import_expression(self, expr)
        }
        fn visit_call_expression(&mut self, expr: &oxc_ast::ast::CallExpression<'a>) {
            if expr.arguments.len() == 1 {
                if let Expression::Identifier(callee_id) = &expr.callee {
                    if callee_id.name == "require" {
                        let arg = &expr.arguments[0];
                        if let Argument::StringLiteral(source) = arg {
                            self.specifiers.push((source.value.as_str(), source.span));
                        } else {
                            self.non_literal_imports.push(arg.span());
                        }
                    }
                };
            }
            walk_call_expression(self, expr)
        }
    }

    let mut visitor = ImportsVisitor::<'a>::default();
    walk_program(&mut visitor, &parse_return.program);
    (
        Imports {
            specifiers: visitor.specifiers,
            non_literal_imports: visitor.non_literal_imports,
        },
        parse_return.errors,
    )
}

#[cfg(test)]
mod tests {

    use super::parse_imports;
    use oxc_allocator::Allocator;
    use oxc_span::SourceType;

    // fn collect_deps(src: &str) -> Result<(Vec<String>, Vec<Span>), Vec<OxcDiagnostic>> {
    //     let mut deps: Vec<String> = vec![];
    //     let mut dynamic_import_spans: Vec<Span> = vec![];
    //     parse_imports(
    //         &Default::default(),
    //         src,
    //         |dep| deps.push(dep.to_owned()),
    //         |span| dynamic_import_spans.push(span),
    //     )?;
    //     Ok((deps, dynamic_import_spans))
    // }
    #[test]
    fn test_get_deps() {
        let allocator = Allocator::default();
        let src = "import 'foo';
import a from 'a';
import type b from 'b';
import c = require('c');
import type bar = require('bar');
const d = import('d');
const e = import('e' + d);
const f = require('f');
const g = require('g' + f);
";
        let imports = parse_imports(&allocator, SourceType::default().with_module(true), src).0;
        assert_eq!(
            imports
                .specifiers
                .into_iter()
                .map(|(s, _)| s)
                .collect::<Vec<_>>(),
            vec!["foo", "a", "c", "d", "f"]
        );
        assert_eq!(
            imports
                .non_literal_imports
                .into_iter()
                .map(|span| span.source_text(&src))
                .collect::<Vec<&str>>(),
            vec!["'e' + d", "'g' + f"]
        )
    }
}
