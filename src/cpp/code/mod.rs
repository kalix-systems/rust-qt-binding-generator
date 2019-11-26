use super::helpers::*;
use super::*;
mod model_getter_setter;
use model_getter_setter::write_model_getter_setter;

mod object;
use object::write_cpp_object;

/// Entry point for producing the
/// generated C++ code
pub fn write_cpp(conf: &Config) -> Result<()> {
    let mut write_buf = Vec::new();

    let mut h_file = conf.out_dir.join(&conf.cpp_file);
    h_file.set_extension("h");
    let file_name = h_file.file_name().unwrap().to_string_lossy();

    // print header
    writeln!(
        write_buf,
        "/* generated by riqtshaw */\n#include \"{}\"",
        file_name
    )?;

    block(
        &mut write_buf,
        "namespace",
        "",
        |write_buf, _| {
            for option in conf.optional_types() {
                if option != "QString" && option != "QByteArray" {
                    writeln!(
                        write_buf,
                        "
        struct option_{} {{
        public:
            {0} value;
            bool some;
            operator QVariant() const {{
                if (some) {{
                    return QVariant::fromValue(value);
                }}
                return QVariant();
            }}
        }};
        static_assert(std::is_pod<option_{0}>::value, \"option_{0} must be a POD type.\");",
                        option
                    )?;
                }
            }
            writeln!(write_buf, include_str!("../cpp/complex_types.cpp_string"))?;

            for (name, object) in conf.objects.iter() {
                for prop_name in object.non_object_property_names() {
                    writeln!(
                        write_buf,
                        "inline void {fn_name}({class_name}* o)",
                        fn_name = changed_f(object, prop_name),
                        class_name = name
                    )?;
                    writeln!(write_buf, "{{\nQ_EMIT o->{}Changed();\n}}", prop_name)?;
                }
            }
            Ok(())
        },
        (),
    )?;

    for o in conf.objects.values() {
        if o.object_type != ObjectType::Object {
            write_cpp_model(&mut write_buf, o)?;
        }

        block(
            &mut write_buf,
            "extern \"C\"",
            "",
            |write_buf, _| {
                write_object_c_decl(write_buf, o, conf)?;
                Ok(())
            },
            (),
        )?;
    }

    for o in conf.objects.values() {
        write_cpp_object(&mut write_buf, o, conf)?;
    }

    let file = conf.out_dir.join(&conf.cpp_file);
    write_if_different(file, &write_buf)
}

fn write_cpp_model(w: &mut Vec<u8>, o: &Object) -> Result<()> {
    let lcname = snake_case(&o.name);

    let index = if o.object_type == ObjectType::Tree {
        "index.internalId()"
    } else {
        "index.row()"
    };

    writeln!(w, "extern \"C\" {{")?;

    define_ffi_getters(o, w)?;

    writeln!(
        w,
        "void {}_sort({}::Private*, unsigned char column, Qt::SortOrder order = Qt::AscendingOrder);",
        lcname,
        o.name
    )?;

    if o.object_type == ObjectType::List {
        writeln!(
            w,
            include_str!("../cpp/list_member_fn_defs.cpp_string"),
            name = o.name,
            lowercase_name = lcname,
            column_count = o.column_count()
        )?;
    } else {
        writeln!(
            w,
            include_str!("../cpp/tree_member_fn_defs.cpp_string"),
            o.name,
            lcname,
            o.column_count()
        )?;
    }

    writeln!(
        w,
        "
void {0}::sort(int column, Qt::SortOrder order)
{{
    {1}_sort(m_d, column, order);
}}",
        o.name, lcname
    )?;

    write_abstract_item_flags_function(o, w)?;

    for ip in o.item_properties.iter() {
        write_model_getter_setter(w, index, ip.0, ip.1, o)?;
    }

    writeln!(
        w,
        "QVariant {}::data(const QModelIndex &index, int role) const
{{
    Q_ASSERT(rowCount(index.parent()) > index.row());
    switch (index.column()) {{",
        o.name
    )?;

    for col in 0..o.column_count() {
        writeln!(w, "    case {}:", col)?;

        writeln!(w, "        switch (role) {{")?;

        for (i, (name, ip)) in o.item_properties.iter().enumerate() {
            let empty = Vec::new();
            let roles = ip.roles.get(col).unwrap_or(&empty);
            if col > 0 && roles.is_empty() {
                continue;
            }

            for role in roles {
                writeln!(w, "        case Qt::{}:", role_name(role))?;
            }

            writeln!(w, "        case Qt::UserRole + {}:", i)?;

            let ii = if o.object_type == ObjectType::List {
                ".row()"
            } else {
                ""
            };

            if ip.optional && !ip.is_complex() {
                writeln!(w, "return {}(index{});", name, ii)?;
            } else if ip.optional {
                writeln!(
                    w,
                    "return cleanNullQVariant(QVariant::fromValue({}(index{})));",
                    name, ii
                )?;
            } else {
                writeln!(w, "return QVariant::fromValue({}(index{}));", name, ii)?;
            }
        }

        writeln!(w, "}} break;")?;
    }

    writeln!(
        w,
        "    }}
    return QVariant();
}}"
    )?;

    write_abstract_item_role_function(o, w)?;

    writeln!(
    w,"
QVariant {0}::headerData(int section, Qt::Orientation orientation, int role) const
{{
    if (orientation != Qt::Horizontal) {{
        return QVariant();
    }}
    return m_headerData.value(qMakePair(section, static_cast<Qt::ItemDataRole>(role)), role == Qt::DisplayRole ?QString::number(section + 1) :QVariant());
}}

bool {0}::setHeaderData(int section, Qt::Orientation orientation, const QVariant &value, int role)
{{
    if (orientation != Qt::Horizontal) {{
        return false;
    }}
    m_headerData.insert(qMakePair(section, static_cast<Qt::ItemDataRole>(role)), value);
    return true;
}}
",
        o.name
    )?;

    if model_is_writable(o) {
        writeln!(
            w,
            "bool {}::setData(const QModelIndex &index, const QVariant &value, int role)\n{{",
            o.name
        )?;

        for col in 0..o.column_count() {
            if !is_column_write(o, col) {
                continue;
            }

            writeln!(w, "    if (index.column() == {}) {{", col)?;

            for (i, (name, ip)) in o.item_properties.iter().enumerate() {
                if !ip.write {
                    continue;
                }

                let empty = Vec::new();

                let roles = ip.roles.get(col).unwrap_or(&empty);

                if col > 0 && roles.is_empty() {
                    continue;
                }

                write!(w, "        if (")?;

                for role in roles {
                    write!(w, "role == Qt::{} || ", role_name(role))?;
                }

                writeln!(w, "role == Qt::UserRole + {}) {{", i)?;

                let ii = if o.object_type == ObjectType::List {
                    ".row()"
                } else {
                    ""
                };

                if ip.optional && !ip.is_complex() {
                    writeln!(
                        w,
                        "            return set{}(index{}, value);",
                        upper_initial(name),
                        ii
                    )?;
                } else {
                    let pre = if ip.optional {
                        "!value.isValid() || value.isNull() ||"
                    } else {
                        ""
                    };

                    writeln!(
                        w,
                        "            if ({}value.canConvert(qMetaTypeId<{}>())) {{",
                        pre,
                        ip.type_name()
                    )?;

                    writeln!(
                        w,
                        "                return set{}(index{}, value.value<{}>());",
                        upper_initial(name),
                        ii,
                        ip.type_name()
                    )?;

                    writeln!(w, "}}")?;
                }

                writeln!(w, "}}")?;
            }

            writeln!(w, "}}")?;
        }

        writeln!(w, "return false;\n}}\n")?;
    }

    Ok(())
}
