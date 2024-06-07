use std::{str::FromStr, sync::Arc};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use kube::{
    api::{Patch, PatchParams},
    config::NamedContext,
    CustomResource, ResourceExt,
};
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, SinglePart},
    Address, AsyncTransport,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::operator::{
    ctx::OperatorCtx,
    error::{KuoError, KuoResult},
    utils::get_kube_cert,
};

use super::inline_permissions::InlinePermissions;

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(
    group = "kuo.github.io",
    version = "v1",
    kind = "ManagedUser",
    status = "ManagedUserStatus",
    printcolumn = r#"
    {
        "name":"Email", 
        "type":"string", 
        "description":"User's email", 
        "jsonPath":".spec.email"
    }, 
    {
        "name": "Full Name", 
        "type": "string", 
        "description": "User's real name", 
        "jsonPath": ".spec.full_name"
    }
    "#
)]
pub struct ManagedUserCRD {
    /// Email to use for sending kubeconfig
    #[validate(email)]
    #[schemars(schema_with = "immutable_rule::<String>")]
    pub email: String,
    /// User's full name. Used in email.
    pub full_name: Option<String>,
    /// List of inlined permissions.
    pub inline_permissions: Option<InlinePermissions>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct ManagedUserStatus {
    /// This is a private key, used by the client.
    /// This key is used for issuing certificate sign requests.
    pub pkey: String,
    /// Resulting certificate for a user.
    pub cert: Option<String>,
}

pub fn immutable_rule<T: JsonSchema>(
    gen: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    let schema = gen.subschema_for::<T>();
    let mut val = serde_json::to_value(schema).unwrap();
    let obj = val.as_object_mut().unwrap();
    obj.insert(
        String::from("x-kubernetes-validations"),
        serde_json::json!([
            {
                "rule": "self == oldSelf",
                "message": "Cannot change field. The value is immutable."
            }
        ]),
    );
    serde_json::from_value(val).unwrap()
}

impl ManagedUser {
    pub async fn update_status(
        &self,
        new_status: &ManagedUserStatus,
        ctx: Arc<OperatorCtx>,
    ) -> KuoResult<ManagedUser> {
        tracing::info!("Updating status. {}", self.name_any());
        let api = kube::Api::<ManagedUser>::all(ctx.client.clone());
        let updated_user = api
            .patch_status(
                &self.name_any(),
                &PatchParams::default(),
                &Patch::Merge(serde_json::json!({"status": new_status})),
            )
            .await?;
        Ok(updated_user)
    }

    pub async fn build_kubeconfig(
        &self,
        ctx: Arc<OperatorCtx>,
    ) -> KuoResult<kube::config::Kubeconfig> {
        let Some(ManagedUserStatus {
            pkey: private_key,
            cert: Some(client_cert),
        }) = &self.status
        else {
            return Err(KuoError::CannotGenerateKubeconfig(format!(
                "User {} doesn't have an approved certificate.",
                self.name_any()
            )));
        };
        let root_kube_cert = get_kube_cert(ctx.clone()).await?;
        let mut kubeconfig = kube::config::Kubeconfig::default();
        kubeconfig.clusters.push(kube::config::NamedCluster {
            name: String::from("cluster"),
            cluster: Some(kube::config::Cluster {
                server: Some(String::from(ctx.args.kube_addr.clone())),
                certificate_authority_data: Some(BASE64_STANDARD.encode(root_kube_cert)),
                ..Default::default()
            }),
        });
        kubeconfig.auth_infos.push(kube::config::NamedAuthInfo {
            name: String::from(self.name_any()),
            auth_info: Some(kube::config::AuthInfo {
                client_certificate_data: Some(BASE64_STANDARD.encode(client_cert)),
                client_key_data: Some(BASE64_STANDARD.encode(private_key).into()),
                ..Default::default()
            }),
        });
        kubeconfig.contexts.push(NamedContext {
            name: String::from("default"),
            context: Some(kube::config::Context {
                cluster: String::from("cluster"),
                user: String::from(self.name_any()),
                ..Default::default()
            }),
        });
        kubeconfig.current_context = Some(String::from("default"));
        Ok(kubeconfig)
    }

    pub async fn send_kubeconfig(&self, ctx: Arc<OperatorCtx>) -> KuoResult<()> {
        let kubeconfig = self.build_kubeconfig(ctx.clone()).await?;
        let kube_config_attachement = Attachment::new(String::from("kubeconfig.yaml"))
            .body(serde_yaml::to_string(&kubeconfig)?, ContentType::TEXT_PLAIN);
        let msg = lettre::Message::builder()
            .from(Mailbox::new(
                Some(ctx.args.smtp_from_name.clone()),
                lettre::Address::from_str(&ctx.args.smtp_from_email)?,
            ))
            .to(Mailbox::new(
                self.spec.full_name.clone(),
                Address::from_str(self.spec.email.as_str())?,
            ))
            .subject("You've beed added to the Kubernetes cluster!")
            .date_now()
            .multipart(lettre::message::MultiPart::mixed().singlepart(SinglePart::html(format!(
                "Hello, <b>{}</b>! You've been added to the Kubernetes cluster. Please download the kubeconfig.",
                self.name_any(),
            )
            )).singlepart(kube_config_attachement))?;
        ctx.smtp.send(msg).await?;
        Ok(())
    }

    pub async fn sync_permissions(&self, ctx: Arc<OperatorCtx>) -> KuoResult<()> {
        tracing::info!("Syncing permissions for user {}", self.name_any());
        if let Some(permissions) = &self.spec.inline_permissions {
            permissions.apply(self, ctx.clone()).await?;
        }
        Ok(())
    }
}
