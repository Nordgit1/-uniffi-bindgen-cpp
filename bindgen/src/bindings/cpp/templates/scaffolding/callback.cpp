{%- let ffi_converter_name = typ|ffi_converter_name %}
{%- let type_name = typ|type_name %}
{%- let class_name = type_name|class_name %}
{%- let canonical_type_name = typ|canonical_name %}
{%- let iface = ci|get_callback_interface_definition(name) %}
namespace {
class {{ iface.name() }}Proxy: public {{ iface.name() }} {
        public:
            {{ iface.name() }}Proxy(uint64_t handle): handle(handle) { }

            ~{{ iface.name() }}Proxy() override {
                ForeignCallback *callback_stub = reinterpret_cast<ForeignCallback *>({{ ffi_converter_name|class_name }}::fn_handle.load());

                callback_stub(this->handle, 0, nullptr, 0, nullptr);
            }

            {% for m in iface.methods() %}
            {%- match m.return_type() -%}
            {% when Some with (return_type) %}{{ return_type|type_name }} {% when None %}void {% endmatch %}{{ m.name() }}(
            {%- for arg in m.arguments() %}
            {{- arg|parameter }} {% if !loop.last %}, {% endif -%}
            {% endfor %}) override {
                ForeignCallback *callback_stub = reinterpret_cast<ForeignCallback *>({{ ffi_converter_name|class_name }}::fn_handle.load());
                if (callback_stub == nullptr) {
                    throw std::runtime_error("callback stub not initialized, this indicates a bug in the generated code");
                }

                RustBuffer out_buf = {
                    .capacity = 0,
                    .len = 0,
                    .data = nullptr,
                };

                int32_t size = 0;
                {% for arg in m.arguments() %}
                size += {{ arg|allocation_size_fn }}({{ arg.name() }});
                {% endfor %}
                RustBuffer in_buf = rustbuffer_alloc(size);
                RustStream in_stream(&in_buf);

                {% for arg in m.arguments() %}
                {{ arg|write_fn }}(in_stream, {{ arg.name() }});
                {% endfor %}

                auto ret = callback_stub(this->handle, {{ loop.index }}, in_buf.data, size, &out_buf);

                rustbuffer_free(in_buf);

                {% match m.return_type() %}
                {% when Some with (return_type) %}
                RustStream out_stream(&out_buf);
                auto result = {{ return_type|read_fn }}(out_stream);
                rustbuffer_free(out_buf);

                return result;
                {% else %}
                rustbuffer_free(out_buf);
                {% endmatch %}
            }
            {% endfor %}
        private:
            uint64_t handle;
    };
}

{{ type_name }} {{ ffi_converter_name|class_name }}::lift(uint64_t handle) {
    return std::make_shared<{{ iface.name() }}Proxy>(handle);
}

uint64_t {{ ffi_converter_name|class_name }}::lower(const {{ type_name }} &obj) {
    return {{ ffi_converter_name|class_name }}::fn_handle.load();
}

{{ type_name }} {{ ffi_converter_name|class_name }}::read(RustStream &stream) {
    uint64_t handle;
    stream >> handle;

    return {{ ffi_converter_name|class_name }}::lift(handle);
}

void {{ ffi_converter_name|class_name }}::write(RustStream &stream, const {{ type_name }} &obj) {
    stream << {{ ffi_converter_name|class_name }}::lower(obj);
}

int32_t {{ ffi_converter_name|class_name }}::allocation_size(const {{ type_name }} &obj) {
    return sizeof(uint64_t);
}

std::atomic<uint64_t> {{ ffi_converter_name|class_name }}::fn_handle = 0;


