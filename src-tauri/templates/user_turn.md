<terminal>
{%- if !terminal.is_empty() %}
{{ terminal }}
{%- endif %}
</terminal>
{% if !message.is_empty() -%}
<user_message>{{ message }}</user_message>
{%- endif %}
