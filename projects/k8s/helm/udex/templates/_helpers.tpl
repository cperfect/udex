{{/*
  _helpers.tpl — named template library for the udex chart.

  Helm named templates work like functions: define them here with
  {{- define "udex.name" -}} ... {{- end }}, then call them anywhere in
  the chart with {{ include "udex.name" . }} where "." passes the current
  context (values, release metadata, etc.) into the template.

  The `trunc 63 | trimSuffix "-"` pipeline enforces the Kubernetes label
  value limit of 63 characters and removes any trailing hyphens.
*/}}

{{/*
Expand the name of the chart.
*/}}
{{- define "udex.name" -}}
{{- .Chart.Name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
Combines the Helm release name with the chart name, e.g. "udex-udex".
Using both avoids name collisions when multiple releases share a namespace.
*/}}
{{- define "udex.fullname" -}}
{{- printf "%s-%s" .Release.Name .Chart.Name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels applied to every resource in the chart.
helm.sh/chart enables "helm list" to identify which chart owns a resource.
*/}}
{{- define "udex.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | quote }}
{{ include "udex.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels used by the Deployment to find its Pods and by the Service
to route traffic to them. These must be stable across upgrades — changing
selector labels requires deleting and recreating the Deployment.
*/}}
{{- define "udex.selectorLabels" -}}
app.kubernetes.io/name: {{ include "udex.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
