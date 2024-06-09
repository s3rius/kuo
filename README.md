# Kubernetes User Operator

Simple kubernetes operator for managing users in a cluster.
Basically, it gives you simple CRD that allows you to add new users to the cluster, or manage existing ones.

## Installation

Easiest way to install the operator is to use the provided helm chart.
```bash
helm show values oci://ghcr.io/s3rius/kuo/kuo > values.yaml
# Edit values.yaml to suit your needs
helm install kuo oci://ghcr.io/s3rius/kuo/kuo -f values.yaml
```


## Usage

To create a new user, you need to create a new `ManagedUser` object in the cluster. For example:

```yaml
apiVersion: kuo.github.io/v1
kind: ManagedUser
metadata:
  name: s3rius
spec: {}
```

This will create a new user with the name `s3rius` in the cluster. Once the user is created, operator
will try to create a CertificateSigningRequest for the user, and approve it. After that, the generated
kubeconfig will be stored in the `ManagedUser/status/kubeconfig` field.

To get the generated kubeconfig, you can use the following command:

```bash
kubectl get  managedusers.kuo.github.io s3rius -o=jsonpath="{.status.kubeconfig}"
```

This will output the kubeconfig for the user `s3rius`.

### Permissions

Also, you can inline the permissions for the user in the `ManagedUser` object. It's highly encouraged to use the inline permissions, because they are managed by the operator, and will be automatically updated if the permissions change on the `ManagedUser`.

For example:

```yaml
apiVersion: kuo.github.io/v1
kind: ManagedUser
metadata:
  name: s3rius
spec:
  inlinePermissions:
    clusterPermissions:
      - apiGroups: ["apps"]
        resources: ["deployments"]
        verbs: ["get", "list"]
    namespacedPermissions:
      - namespace: default
        permissions:
          - apiGroups: [""]
            resources: ["configmaps"]
            verbs: ["get", "list"]
```

This config will create appropriate `Role`, `ClusterRole`, `RoleBinding` and `ClusterRoleBinding` objects in the cluster, and will grant specified permissions to the user `s3rius`.

If you will change the permissions in the `ManagedUser` object, the operator will automatically update the permissions for the user.

### Deleting the user

If you delete the `ManagedUser` object, all associated permissions will be automatically removed from the cluster. But if you created any rolebindings or clusterrolebindings manually, you need to remove them manually.

### Email notifications

If you want to send an email with the generated kubeconfig, you need to setup `SMTP` configuration and then you will be able to specify the `email` field in the `ManagedUser` object. For example:

```yaml
apiVersion: kuo.github.io/v1
kind: ManagedUser
metadata:
  name: s3rius
spec:
  email: s3riussan@gmail.com
```

This will send an email with the kubeconfig to the email address `s3riussan@gmail.com` once the kubeconfig is created.


## Configuration

```
Usage: kuo-operator [OPTIONS]

Options:
      --signer-name <SIGNER_NAME>
          Name of the signer which should sign all certificate signing requests created by the operator [env: KUO_OPERATOR_SIGNER_NAME=] [default: kubernetes.io/kube-apiserver-client]
      --kube-addr <KUBE_ADDR>
          Kubernetes API server host [env: KUO_OPERATOR_KUBE_ADDR=https://0.0.0.0:37995] [default: https://0.0.0.0:6443]
      --default-cert-name <DEFAULT_CERT_NAME>
          Name of the configmap which contains the kube root certificate authority. This certificate authority will be used to verify the kube api server [env: DEFAULT_CERT_CM_NAME=] [default: kube-root-ca.crt]
      --default-cert-key <DEFAULT_CERT_KEY>
          Key of the configmap which contains the kube root certificate authority data [env: DEFAULT_CERT_CM_KEY=] [default: ca.crt]
      --smtp-url <URL>
          SMTP server host. This variable should specify smtp or smtps URL [env: KUO_OPERATOR_SMTP_URL=smtp://mail.le-memese.com?tls=required]
      --smtp-port <PORT>
          SMTP server port [env: KUO_OPERATOR_SMTP_PORT=587] [default: 587]
      --smtp-user <USER>
          SMTP username to authenticate with [env: KUO_OPERATOR_SMTP_USER=kuo@le-memese.com] [default: kum]
      --smtp-password <PASSWORD>
          SMTP password to authenticate with [env: KUO_OPERATOR_SMTP_PASS=123321] [default: kum]
      --smtp-from-email <FROM_EMAIL>
          [env: KUO_OPERATOR_SMTP_FROM_EMAIL=kuo@le-memese.com]
      --smtp-from-name <FROM_NAME>
          [env: KUO_OPERATOR_SMTP_FROM_NAME=] [default: "Kubernetes User Operator"]
  -h, --help
          Print help
  -V, --version
          Print version
```