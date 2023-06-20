{{- define "operator.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "operator.fullname" -}}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- $name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "operator.labels" -}}
{{ include "operator.selectorLabels" . }}
app.kubernetes.io/name: {{ include "operator.name" . }}
app.kubernetes.io/version: {{ .Values.image.tag | default .Chart.AppVersion | quote }}
{{- end }}

{{- define "operator.selectorLabels" -}}
app: {{ include "operator.name" . }}
{{- end }}
