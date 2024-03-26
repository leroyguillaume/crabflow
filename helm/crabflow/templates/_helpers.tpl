{{- define "crabflow.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "crabflow.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "crabflow.databaseEnvVars" -}}
- name: DATABASE_HOST
  value: {{ .Values.database.host }}
{{- if .Values.database.secret.name }}
- name: DATABASE_PASSWORD
  valueFrom:
    secretKeyRef:
        name: {{ .Values.database.secret.name }}
        key: {{ .Values.database.secret.key }}
{{- end }}
{{- end }}

{{- define "crabflow.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "crabflow.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{- define "crabflow.common.labels" -}}
helm.sh/chart: {{ include "crabflow.chart" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "crabflow.common.selectorLabels" -}}
app.kubernetes.io/name: {{ include "crabflow.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "crabflow.builder.fullname" -}}
{{- if .Values.builder.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default (printf "%s-builder" .Chart.Name) .Values.builder.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "crabflow.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "crabflow.builder.labels" -}}
{{ include "crabflow.common.labels" . }}
{{ include "crabflow.builder.selectorLabels" . }}
{{- end }}

{{- define "crabflow.builder.selectorLabels" -}}
{{ include "crabflow.common.selectorLabels" . }}
app.kubernetes.io/component: builder
{{- end }}
