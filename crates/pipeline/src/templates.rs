//! Template engine trait for the CogWorks pipeline domain.
//!
//! The pipeline domain uses templates to render structured prompts, GitHub
//! comments, and PR bodies. The [`TemplateEngine`] trait abstracts over the
//! templating library so the pipeline crate has no dependency on a specific
//! implementation (e.g. Handlebars, Tera, Minijinja).
//!
//! ## Architectural Layer
//!
//! Infrastructure crates implement [`TemplateEngine`];
//! the `pipeline` crate only uses the trait.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/github-traits.md` §TemplateEngine for the full
//! contract and variable-naming conventions.

use std::collections::HashMap;

use async_trait::async_trait;
use thiserror::Error;

// ─── Error type ─────────────────────────────────────────────────────────────

/// Errors returned by [`TemplateEngine`] operations.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// The requested template name is not registered with this engine.
    #[error("template not found: {name}")]
    NotFound {
        /// The template name that was requested.
        name: String,
    },

    /// One or more required template variables were missing from the provided
    /// context map.
    #[error("template render failed: missing variables: {missing:?}")]
    MissingVariables {
        /// Names of the variables that were required but absent.
        missing: Vec<String>,
    },

    /// The template source contains a syntax error.
    ///
    /// Typically indicates a misconfigured template file; not a runtime error
    /// in normal operation.
    #[error("template syntax error in '{name}': {message}")]
    SyntaxError {
        /// Name of the template with the syntax error.
        name: String,
        /// Human-readable description of the syntax error.
        message: String,
    },

    /// Template rendering produced output that violates an expected constraint
    /// (e.g. empty output, output exceeding a size limit).
    #[error("template render constraint violated for '{name}': {message}")]
    ConstraintViolation {
        /// Name of the template that was rendered.
        name: String,
        /// Human-readable description of the violated constraint.
        message: String,
    },
}

// ─── Trait ──────────────────────────────────────────────────────────────────

/// Template rendering and introspection.
///
/// Templates are pre-loaded at startup by the infrastructure implementation
/// (e.g. read from `.cogworks/templates/`). The pipeline domain calls
/// [`TemplateEngine::render`] with a context map of variable names to string
/// values.
///
/// All variable values are `String` to keep the interface simple; the template
/// system performs any further formatting.
///
/// ## Specification
///
/// See `docs/spec/interfaces/github-traits.md` §TemplateEngine.
#[async_trait]
pub trait TemplateEngine: Send + Sync {
    /// Render a named template with the provided variable context.
    ///
    /// # Arguments
    ///
    /// * `name` — the template identifier (e.g. `"pr-body"`, `"node-comment"`).
    /// * `context` — a map of variable name → string value. All variables
    ///   declared as required by the template must be present.
    ///
    /// # Returns
    ///
    /// The rendered string (Markdown, plain text, or JSON — depending on the
    /// template).
    ///
    /// # Errors
    ///
    /// - [`TemplateError::NotFound`] — unknown `name`.
    /// - [`TemplateError::MissingVariables`] — required variables absent from
    ///   `context`.
    /// - [`TemplateError::SyntaxError`] — template has an unrecoverable syntax
    ///   error (should not occur in production if templates pass CI validation).
    /// - [`TemplateError::ConstraintViolation`] — rendered output violated a
    ///   post-render constraint.
    async fn render(
        &self,
        name: &str,
        context: HashMap<String, String>,
    ) -> Result<String, TemplateError>;

    /// Return the list of variable names that the named template requires.
    ///
    /// Used by the pipeline to validate that all necessary context data is
    /// assembled before calling [`TemplateEngine::render`].
    ///
    /// # Errors
    ///
    /// - [`TemplateError::NotFound`] — unknown `name`.
    async fn list_required_variables(&self, name: &str) -> Result<Vec<String>, TemplateError>;
}
