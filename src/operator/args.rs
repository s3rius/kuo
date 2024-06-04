#[derive(clap::Parser, Debug, Clone)]
#[clap(name = "kuo-operator", version, author, about)]
pub struct OperatorArgs {
    /// Name of the signer which should sign all
    /// certificate signing requests created by the operator.
    #[clap(
        long,
        env = "KUO_OPERATOR_SIGNER_NAME",
        default_value = "kubernetes.io/kube-apiserver-client"
    )]
    pub signer_name: String,

    #[clap(long, env = "KUO_OPERATOR_SERVER_HOST", default_value = "0.0.0.0")]
    pub server_host: String,

    #[clap(long, env = "KUO_OPERATOR_SERVER_PORT", default_value = "8000")]
    pub server_port: u16,

    /// Kubernetes API server host.
    #[clap(long, env = "KUO_OPERATOR_KUBE_ADDR", default_value = "https://0.0.0.0:6443")]
    pub kube_addr: String,

    #[clap(long, env = "DEFAULT_CERT_CM_NAME", default_value = "kube-root-ca.crt")]
    pub default_cert_name: String,

    #[clap(long, env = "DEFAULT_CERT_CM_KEY", default_value = "ca.crt")]
    pub default_cert_key: String,

    /// SMTP server host.
    /// This variable should specify smtp or smtps URL.
    #[clap(long, env = "KUO_OPERATOR_SMTP_URL")]
    pub smtp_url: String,

    /// SMTP server port.
    #[clap(long, env = "KUO_OPERATOR_SMTP_PORT", default_value = "587")]
    pub smtp_port: u16,

    /// SMTP username to authenticate with.
    #[clap(long, env = "KUO_OPERATOR_SMTP_USER", default_value = "kum")]
    pub smtp_user: String,

    /// SMTP password to authenticate with.
    #[clap(long, env = "KUO_OPERATOR_SMTP_PASS", default_value = "kum")]
    pub smtp_pass: String,

    #[clap(long, env = "KUO_OPERATOR_SMTP_FROM_EMAIL")]
    pub smtp_from_email: String,

    #[clap(
        long,
        env = "KUO_OPERATOR_SMTP_FROM_NAME",
        default_value = "Kubernetes User Operator"
    )]
    pub smtp_from_name: String,
}
