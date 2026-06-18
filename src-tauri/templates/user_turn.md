{% if let Some(terminal) = terminal -%}
<terminal>
{{ terminal }}
</terminal>
{% endif -%}
<commandline>{{ commandline.unwrap_or("") }}</commandline>
{% if !msg.is_empty() -%}
<msg>{{ msg }}</msg>
{%- endif %}
