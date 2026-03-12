use std::path::Path;

use crate::{DeployPlan, DeploymentBundle, ProviderKind, Result};

pub trait Deployer {
    fn provider(&self) -> ProviderKind;

    fn build_plan(&self, bundle: &DeploymentBundle) -> Result<DeployPlan>;

    fn materialize(&self, bundle: &DeploymentBundle, output_dir: &Path) -> Result<DeployPlan> {
        let plan = self.build_plan(bundle)?;
        plan.write_to(output_dir)?;
        Ok(plan)
    }
}
