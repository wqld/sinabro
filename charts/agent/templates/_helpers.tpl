{{- define "agent.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "agent.fullname" -}}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- $name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "agent.labels" -}}
{{ include "agent.selectorLabels" . }}
app.kubernetes.io/name: {{ include "agent.name" . }}
app.kubernetes.io/version: {{ .Values.image.tag | default .Chart.AppVersion | quote }}
{{- end }}

{{- define "agent.selectorLabels" -}}
app: {{ include "agent.name" . }}
{{- end }}
