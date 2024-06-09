#[derive(clap::Args, Debug, Clone)]
#[group(requires = "url", requires = "from_email")]
pub struct SMTPArgs {
    /// SMTP server host.
    /// This variable should specify smtp or smtps URL.
    #[clap(long = "smtp-url", env = "KUO_OPERATOR_SMTP_URL", required = false)]
    pub url: String,

    /// SMTP server port.
    #[clap(
        long = "smtp-port",
        env = "KUO_OPERATOR_SMTP_PORT",
        default_value = "587"
    )]
    pub port: u16,

    /// SMTP username to authenticate with.
    #[clap(
        long = "smtp-user",
        env = "KUO_OPERATOR_SMTP_USER",
        default_value = "kum"
    )]
    pub user: String,

    /// SMTP password to authenticate with.
    #[clap(
        long = "smtp-password",
        env = "KUO_OPERATOR_SMTP_PASS",
        default_value = "kum"
    )]
    pub password: String,

    #[clap(
        long = "smtp-from-email",
        env = "KUO_OPERATOR_SMTP_FROM_EMAIL",
        required = false
    )]
    pub from_email: String,

    #[clap(
        long = "smtp-from-name",
        env = "KUO_OPERATOR_SMTP_FROM_NAME",
        default_value = "Kubernetes User Operator"
    )]
    pub from_name: String,
}

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

    /// Kubernetes API server host.
    #[clap(
        long,
        env = "KUO_OPERATOR_KUBE_ADDR",
        default_value = "https://0.0.0.0:6443"
    )]
    pub kube_addr: String,

    /// Name of the configmap which contains the kube root certificate authority.
    /// This certificate authority will be used to verify the kube api server.
    #[clap(long, env = "DEFAULT_CERT_CM_NAME", default_value = "kube-root-ca.crt")]
    pub default_cert_name: String,

    /// Key of the configmap which contains the kube root certificate authority data.
    #[clap(long, env = "DEFAULT_CERT_CM_KEY", default_value = "ca.crt")]
    pub default_cert_key: String,

    #[clap(flatten)]
    pub smtp_args: Option<SMTPArgs>,
}
