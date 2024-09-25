use std::{
    collections::HashSet,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use k8s_openapi::{
    api::rbac::v1::{ClusterRole, ClusterRoleBinding, PolicyRule, Role, RoleBinding},
    Resource,
};
use kube::{
    api::{DeleteParams, ListParams, ObjectMeta},
    ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::operator::{
    ctx::OperatorCtx,
    error::KuoResult,
    utils::{meta::ObjectMetaKuoExt, resource::KuoResourceExt},
};

use super::managed_user::ManagedUser;

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    /// `APIGroups` is the name of the `APIGroup` that contains the resources.
    /// If multiple API groups are specified,
    /// any action requested against one of the enumerated resources in any
    /// API group will be allowed.
    /// "" represents the core API group and "*" represents all API groups.
    pub api_groups: Option<Vec<String>>,
    /// Resources is a list of resources this rule applies to. '*' represents all resources.
    pub resources: Option<Vec<String>>,
    /// `ResourceNames` is an optional white list of names that the rule applies to.
    /// An empty set means that everything is allowed.
    pub resource_names: Option<Vec<String>>,
    /// `NonResourceURLs` is a set of partial urls that a user should have access to.  *s are allowed, but only as the full, final step in the path Since non-resource URLs are not namespaced, this field is only applicable for `ClusterRoles` referenced from a `ClusterRoleBinding`. Rules can either apply to API resources (such as "pods" or "secrets") or non-resource URL paths (such as "/api"),  but not both.
    pub non_resource_urls: Option<Vec<String>>,
    /// Verbs is a list of Verbs that apply to ALL the `ResourceKinds` contained in this rule. '*' represents all verbs.
    pub verbs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub struct NamespacedPermissions {
    // Name of the namespace to apply permission to.
    pub namespace: String,
    // List of permissions to apply to the namespace.
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InlinePermissions {
    /// List of cluster-wide permissions.
    pub cluster_permissions: Option<Vec<Permission>>,
    /// List of namespaced permissions.
    pub namespaced_permissions: Option<Vec<NamespacedPermissions>>,
}

impl From<Permission> for PolicyRule {
    fn from(p: Permission) -> Self {
        Self {
            api_groups: p.api_groups,
            resources: p.resources,
            resource_names: p.resource_names,
            non_resource_urls: p.non_resource_urls,
            verbs: p.verbs,
        }
    }
}

impl NamespacedPermissions {
    pub async fn apply(&self, user: &ManagedUser, ctx: Arc<OperatorCtx>) -> KuoResult<String> {
        let mut hasher = DefaultHasher::new();
        serde_yaml::to_string(self)?.hash(&mut hasher);
        let name = format!("{}-{}", user.name_any(), hasher.finish());
        // api.get_metadata_opt(name)
        let mut role_metadata = ObjectMeta::default();
        role_metadata.add_owner(user);
        role_metadata.name = Some(name.clone());
        role_metadata.insert_label("kuo.github.com/user", user.name_any());
        let mut new_role = Role {
            metadata: role_metadata,
            rules: Some(
                self.permissions
                    .iter()
                    .cloned()
                    .map(PolicyRule::from)
                    .collect(),
            ),
        };
        new_role = new_role
            .patch_or_create(kube::Api::namespaced(ctx.client.clone(), &self.namespace))
            .await?;
        let mut rb_metadata = ObjectMeta::default();
        rb_metadata.add_owner(&new_role);
        rb_metadata.name = Some(name.clone());
        rb_metadata.insert_label("kuo.github.com/user", user.name_any());
        rb_metadata.namespace = Some(self.namespace.clone());
        let role_binding = RoleBinding {
            metadata: rb_metadata,
            role_ref: k8s_openapi::api::rbac::v1::RoleRef {
                api_group: String::from(Role::GROUP),
                kind: String::from(Role::KIND),
                name: name.clone(),
            },
            subjects: Some(vec![k8s_openapi::api::rbac::v1::Subject {
                kind: "User".to_string(),
                name: user.name_any(),
                namespace: None,
                api_group: None,
            }]),
        };
        role_binding
            .patch_or_create(kube::Api::namespaced(ctx.client.clone(), &self.namespace))
            .await?;
        Ok(name)
    }
}

impl InlinePermissions {
    /// This function will remove all roles that are not in the `known_permissions` set.
    ///
    /// It iterates over all roles in the cluster and deletes the ones that are not in the `known_permissions` set,
    /// and have the label `kuo.github.com/user` set to the username.
    #[allow(clippy::missing_panics_doc)]
    pub async fn remove_unknown_namespaced_roles(
        user: &ManagedUser,
        known_permissions: &HashSet<String>,
        ctx: Arc<OperatorCtx>,
    ) -> KuoResult<()> {
        let roles = kube::Api::<Role>::all(ctx.client.clone())
            .list(&ListParams {
                label_selector: Some(format!("kuo.github.com/user={}", user.name_any())),
                ..Default::default()
            })
            .await?;
        for role in roles {
            if known_permissions.contains(role.name_any().as_str()) {
                continue;
            }
            kube::Api::<Role>::namespaced(
                ctx.client.clone(),
                // SAFETY: We are sure that the namespace is set,
                // because we are listing roles, which are always namespaced.
                role.namespace().as_deref().unwrap(),
            )
            .delete(role.name_any().as_str(), &DeleteParams::default())
            .await?;
        }
        Ok(())
    }

    /// Apply namespaced permissions for a user.
    ///
    /// This function creates roles and role bindings for the user in the specified namespaces.
    /// Also it remembers the names of the created roles in the `known_permissions` set.
    ///
    /// After creating all roles, it will remove all roles that are not in the `known_permissions` set.
    async fn apply_namespaced_permissions(
        &self,
        user: &ManagedUser,
        ctx: Arc<OperatorCtx>,
    ) -> KuoResult<()> {
        let mut known_permissions = HashSet::new();
        if let Some(namespaced_permissions) = &self.namespaced_permissions {
            for namespaced in namespaced_permissions {
                let res = namespaced.apply(user, ctx.clone()).await;
                match res {
                    Ok(name) => {
                        known_permissions.insert(name);
                    }
                    Err(err) => {
                        tracing::warn!("Failed to create namespaced permission. {err}");
                    }
                }
            }
        }
        Self::remove_unknown_namespaced_roles(user, &known_permissions, ctx.clone()).await?;
        Ok(())
    }

    async fn remove_unknown_cluster_roles(
        user: &ManagedUser,
        known_permission: Option<String>,
        ctx: Arc<OperatorCtx>,
    ) -> KuoResult<()> {
        let roles = kube::Api::<ClusterRole>::all(ctx.client.clone())
            .list(&ListParams {
                label_selector: Some(format!("kuo.github.com/user={}", user.name_any())),
                ..Default::default()
            })
            .await?;
        for role in roles {
            if let Some(known_name) = &known_permission {
                if role.name_any() == *known_name {
                    continue;
                }
            }
            kube::Api::<ClusterRole>::all(ctx.client.clone())
                .delete(role.name_any().as_str(), &DeleteParams::default())
                .await?;
        }
        Ok(())
    }

    async fn apply_cluster_permissions(
        &self,
        user: &ManagedUser,
        ctx: Arc<OperatorCtx>,
    ) -> KuoResult<()> {
        let mut known_name = None;
        if let Some(namespaced_permissions) = &self.cluster_permissions {
            let mut hasher = DefaultHasher::new();
            serde_yaml::to_string(namespaced_permissions)?.hash(&mut hasher);
            let name = format!("{}-{}", user.name_any(), hasher.finish());
            known_name = Some(name.clone());
            // api.get_metadata_opt(name)
            let mut role_metadata = ObjectMeta::default();
            role_metadata.add_owner(user);
            role_metadata.name = Some(name.clone());
            role_metadata.insert_label("kuo.github.com/user", user.name_any());
            let mut new_role = ClusterRole {
                metadata: role_metadata,
                rules: Some(
                    namespaced_permissions
                        .iter()
                        .cloned()
                        .map(PolicyRule::from)
                        .collect(),
                ),
                ..Default::default()
            };
            new_role = new_role
                .patch_or_create(kube::Api::all(ctx.client.clone()))
                .await?;
            let mut rb_metadata = ObjectMeta::default();
            rb_metadata.add_owner(&new_role);
            rb_metadata.name = Some(name.clone());
            rb_metadata.insert_label("kuo.github.com/user", user.name_any());
            let role_binding = ClusterRoleBinding {
                metadata: rb_metadata,
                role_ref: k8s_openapi::api::rbac::v1::RoleRef {
                    api_group: String::from("rbac.authorization.k8s.io"),
                    kind: String::from(ClusterRole::KIND),
                    name: name.clone(),
                },
                subjects: Some(vec![k8s_openapi::api::rbac::v1::Subject {
                    kind: "User".to_string(),
                    name: user.name_any(),
                    namespace: None,
                    api_group: None,
                }]),
            };
            role_binding
                .patch_or_create(kube::Api::all(ctx.client.clone()))
                .await?;
        }
        Self::remove_unknown_cluster_roles(user, known_name, ctx.clone()).await?;
        Ok(())
    }

    /// Apply inlined permissions for a user.
    ///
    /// This function will sync all permissions for the user.
    /// But it won't delete any permissions that were not created by this operator.
    pub async fn apply(&self, user: &ManagedUser, ctx: Arc<OperatorCtx>) -> KuoResult<()> {
        self.apply_namespaced_permissions(user, ctx.clone()).await?;
        self.apply_cluster_permissions(user, ctx).await?;
        Ok(())
    }
}
