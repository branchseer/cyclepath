use oxc_allocator::Allocator;
use oxc_ast::{
    ast::{Argument, Expression, ImportOrExportKind},
    visit::{walk::walk_program, Visit},
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
    source: &'a str,
) -> (Imports<'a>, Vec<OxcDiagnostic>) {
    let parser = Parser::new(
        allocator,
        source,
        SourceType::default()
            .with_always_strict(true)
            .with_typescript(true)
            .with_jsx(true)
            .with_module(true),
    );
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
        fn visit_import_declaration(&mut self, decl: &oxc_ast::ast::ImportDeclaration<'a>) {
            if decl.import_kind == ImportOrExportKind::Type {
                return;
            };
            self.specifiers
                .push((decl.source.value.as_str(), decl.source.span))
        }
        fn visit_ts_import_equals_declaration(
            &mut self,
            decl: &oxc_ast::ast::TSImportEqualsDeclaration<'a>,
        ) {
            if decl.import_kind == ImportOrExportKind::Type {
                return;
            };
            if let oxc_ast::ast::TSModuleReference::ExternalModuleReference(
                external_module_reference,
            ) = &decl.module_reference
            {
                let specifier_literal = &external_module_reference.expression;
                self.specifiers
                    .push((specifier_literal.value.as_str(), specifier_literal.span))
            }
        }
        fn visit_import_expression(&mut self, expr: &oxc_ast::ast::ImportExpression<'a>) {
            if let Expression::StringLiteral(string_literal) = &expr.source {
                self.specifiers
                    .push((string_literal.value.as_str(), string_literal.span))
            } else {
                self.non_literal_imports.push(expr.source.span())
            }
        }
        fn visit_call_expression(&mut self, expr: &oxc_ast::ast::CallExpression<'a>) {
            if expr.arguments.len() != 1 {
                return;
            }
            let Expression::Identifier(callee_id) = &expr.callee else {
                return;
            };
            if callee_id.name != "require" {
                return;
            }
            let arg = &expr.arguments[0];
            if let Argument::StringLiteral(source) = arg {
                self.specifiers.push((source.value.as_str(), source.span));
            } else {
                self.non_literal_imports.push(arg.span());
            }
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
        let imports = parse_imports(&allocator, src).0;
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
