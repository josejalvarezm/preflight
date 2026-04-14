pub mod loader;
pub mod logger;
pub mod policy;
pub mod roles;
pub mod router;

use ai_os_shared::boundary::PolicyBoundary;
use ai_os_shared::contract::ContractManifest;
use ai_os_shared::error::{AiOsError, Result};
use ai_os_shared::task::{DecisionLogEntry, TaskDescriptor, TaskResult, TaskStatus};
use policy::{PolicyEngine, PolicyVerdict};
use std::path::Path;

/// The Orchestration Kernel: loads instructions, enforces boundaries, routes tasks, logs decisions.
pub struct Kernel {
    manifest: ContractManifest,
    role_registry: roles::RoleRegistry,
    logger: logger::DecisionLogger,
    policy_engine: PolicyEngine,
}

impl Kernel {
    /// Boot the kernel from a compiled contract manifest file.
    pub fn boot(manifest_path: &Path, log_path: &Path) -> Result<Self> {
        let manifest = loader::load_manifest(manifest_path)?;
        let role_registry = roles::RoleRegistry::from_manifest(&manifest);
        let logger = logger::DecisionLogger::new(log_path)?;
        let policy_engine = PolicyEngine::from_boundaries(manifest.boundaries.clone());

        Ok(Kernel {
            manifest,
            role_registry,
            logger,
            policy_engine,
        })
    }

    /// Boot from an in-memory manifest (useful for testing).
    pub fn boot_from_manifest(manifest: ContractManifest, log_path: &Path) -> Result<Self> {
        let role_registry = roles::RoleRegistry::from_manifest(&manifest);
        let logger = logger::DecisionLogger::new(log_path)?;
        let policy_engine = PolicyEngine::from_boundaries(manifest.boundaries.clone());

        Ok(Kernel {
            manifest,
            role_registry,
            logger,
            policy_engine,
        })
    }

    /// Register a policy boundary with the kernel.
    pub fn add_boundary(&mut self, boundary: PolicyBoundary) {
        self.policy_engine.add_boundary(boundary);
    }

    /// Supersede an existing boundary with a new one.
    pub fn supersede_boundary(
        &mut self,
        old_boundary_id: &str,
        new_boundary: PolicyBoundary,
        authorised_by: &str,
        reason: &str,
    ) -> Option<ai_os_shared::boundary::RuleSupersession> {
        self.policy_engine
            .supersede(old_boundary_id, new_boundary, authorised_by, reason)
    }

    /// Route a task to the appropriate agent.
    ///
    /// **Pre-flight policy check**: Before routing, the task is evaluated against
    /// all active boundaries. If a boundary fires, the task is REFUSED — it never
    /// reaches any agent. The refusal is logged to the diary.
    ///
    /// Returns the agent ID and the instructions for that agent.
    /// The actual agent execution is external — the kernel only decides *who* handles it.
    pub fn route(
        &mut self,
        task: &TaskDescriptor,
    ) -> std::result::Result<router::RoutingDecision, RoutingError> {
        // Phase 1: Policy enforcement (before any routing)
        match self.policy_engine.evaluate(task) {
            PolicyVerdict::Refuse(refusal) => {
                // Log the refusal to the diary
                let entry = DecisionLogEntry {
                    timestamp: chrono::Utc::now(),
                    task_id: task.id.clone(),
                    selected_agent: "POLICY_ENGINE".to_string(),
                    rationale: refusal.reason.clone(),
                    outcome: Some(TaskStatus::Refused),
                    prev_hash: String::new(),
                };
                self.logger.log(&entry).map_err(RoutingError::Routing)?;

                return Err(RoutingError::PolicyRefusal(Box::new(refusal)));
            }
            PolicyVerdict::Allow => {}
        }

        // Phase 2: Normal routing
        let decision = router::route(&self.role_registry, task).map_err(RoutingError::Routing)?;

        // Log the routing decision
        let entry = DecisionLogEntry {
            timestamp: chrono::Utc::now(),
            task_id: task.id.clone(),
            selected_agent: decision.agent_id.clone(),
            rationale: decision.rationale.clone(),
            outcome: None,
            prev_hash: String::new(),
        };
        self.logger.log(&entry).map_err(RoutingError::Routing)?;

        Ok(decision)
    }

    /// Record the outcome of a task after agent execution.
    pub fn record_outcome(&mut self, result: &TaskResult) -> Result<()> {
        let entry = DecisionLogEntry {
            timestamp: chrono::Utc::now(),
            task_id: result.task_id.clone(),
            selected_agent: result.agent_id.clone(),
            rationale: format!("Outcome recorded: {:?}", result.status),
            outcome: Some(result.status.clone()),
            prev_hash: String::new(),
        };
        self.logger.log(&entry)?;
        Ok(())
    }

    /// Get a reference to the loaded manifest.
    pub fn manifest(&self) -> &ContractManifest {
        &self.manifest
    }

    /// Get a reference to the role registry.
    pub fn roles(&self) -> &roles::RoleRegistry {
        &self.role_registry
    }

    /// Get a reference to the policy engine.
    pub fn policy_engine(&self) -> &PolicyEngine {
        &self.policy_engine
    }
}

/// Errors that can occur during routing (distinguishes policy refusals from routing failures).
#[derive(Debug)]
pub enum RoutingError {
    /// The task was refused by the policy engine before it reached any agent.
    PolicyRefusal(Box<ai_os_shared::boundary::RefusalRecord>),
    /// Normal routing error (no matching agent, etc.)
    Routing(AiOsError),
}

impl std::fmt::Display for RoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoutingError::PolicyRefusal(r) => write!(f, "REFUSED: {}", r.reason),
            RoutingError::Routing(e) => write!(f, "Routing error: {e}"),
        }
    }
}

impl std::error::Error for RoutingError {}
