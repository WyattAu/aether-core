//! Policy-as-Code Engine
//!
//! Integrates with Open Policy Agent (OPA) for centralized policy
//! management. Policies are evaluated before actor operations to
//! enforce organisational governance rules.

pub mod engine;
pub mod evaluator;
pub mod rule;

pub use engine::PolicyEngine;
pub use evaluator::{EvaluationContext, EvaluationResult, PolicyEvaluator};
pub use rule::{PolicyEffect, PolicyRule, PolicyScope};
