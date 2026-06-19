{% if let Some(terminal) = terminal -%}
<terminal>
{{ terminal }}
</terminal>
{% endif -%}
<commandline>{{ commandline.unwrap_or("") }}</commandline>
{% if !msg.is_empty() -%}
<user_message>{{ msg }}</user_message>
{%- endif %}
