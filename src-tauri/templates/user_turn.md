<terminal>
{%- if !terminal.is_empty() %}
{{ terminal }}
{%- endif %}
</terminal>
{% if let Some(commandline) = commandline -%}
<commandline>{{ commandline }}</commandline>
{% endif -%}
{% if !message.is_empty() -%}
<user_message>{{ message }}</user_message>
{%- endif %}
