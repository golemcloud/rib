use std::fmt::{Display, Formatter};
use uuid::Uuid;

#[derive(Debug, Default, Hash, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub struct ComponentDependencyKey {
    pub component_name: String,
    pub component_id: Uuid,
    pub component_revision: u64,
    pub root_package_name: Option<String>,
    pub root_package_version: Option<String>,
}

impl Display for ComponentDependencyKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Component: {}, ID: {}, Revision: {}, Root Package: {}@{}",
            self.component_name,
            self.component_id,
            self.component_revision,
            self.root_package_name.as_deref().unwrap_or("unknown"),
            self.root_package_version.as_deref().unwrap_or("unknown")
        )
    }
}
