# Kubernetes User Operator

Simple kubernetes operator for managing users in a cluster.
Basically, it gives you simple CRD that allows you to add new users to the cluster.

```yaml
apiVersion: kuo.github.io/v1
kind: ManagedUser
metadata:
  name: s3rius
spec:
  email: user@gmail.com
```

Applying this manifest will create a new user in the cluster with the name `s3rius` and will send an email to `user@gmail.com` with generated kubeconfig file.