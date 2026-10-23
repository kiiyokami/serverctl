{{/* Port the server listens on: NodePort 30565-30568 maps to 25565-25568, same as the public port. */}}
{{- define "minecraft.port" -}}
{{ sub .Values.nodePort 5000 }}
{{- end }}
