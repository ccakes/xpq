use crate::command::args;
use crate::output::TableOutputWriter;
use crate::reader::ParquetFile;
use clap::{App, Arg, ArgMatches, SubCommand};
use parquet::file::metadata::ParquetMetaDataPtr;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashSet;
use std::io::Write;

pub fn def() -> App<'static, 'static> {
    SubCommand::with_name("sample")
        .about("Randomly sample rows from parquet")
        .arg(
            Arg::with_name("columns")
                .help("Select columns from parquet")
                .takes_value(true)
                .long("columns")
                .multiple(true)
                .short("c"),
        )
        .arg(
            Arg::with_name("sample")
                .validator(args::validate_number)
                .help("Sample size limit")
                .default_value("100")
                .long("sample")
                .short("s"),
        )
        .arg(
            Arg::with_name("format")
                .help("Output format")
                .possible_values(&["table"])
                .default_value("table")
                .long("format")
                .short("f"),
        )
        .arg(
            Arg::with_name("path")
                .validator(args::validate_path)
                .help("Path to parquet")
                .required(true)
                .index(1),
        )
}

fn metadata_headers(
    metadata: &ParquetMetaDataPtr,
    columns: &Option<Vec<String>>,
) -> Vec<String> {
    match columns {
        Some(headers) => headers.clone(),
        None => {
            let file_metadata = metadata.file_metadata();
            let schema = file_metadata.schema();
            let mut headers = Vec::new();

            for field in schema.get_fields() {
                headers.push(String::from(field.name()));
            }

            headers
        }
    }
}

fn sample_indexes(sample: usize, size: usize) -> HashSet<usize> {
    let mut vec = (0..size).collect::<Vec<_>>();
    let mut rng = thread_rng();

    vec.shuffle(&mut rng);

    vec.iter().take(sample).cloned().collect()
}

pub fn run<W: Write>(matches: &ArgMatches, out: &mut W) -> Result<(), String> {
    let columns = args::string_values(matches, "columns")?;
    let sample = args::usize_value(matches, "sample")?;
    let path = args::path_value(matches, "path")?;
    let parquet = ParquetFile::of(path)?;
    let metadata = parquet.metadata(0);
    let rows = parquet.to_row_fmt_iter(columns.clone())?;

    match metadata {
        Some(meta) => {
            let size = parquet.num_rows();
            let headers = metadata_headers(&meta, &columns);
            let indexes = sample_indexes(sample, size);
            let iter = rows
                .enumerate()
                .filter(|t| indexes.contains(&t.0))
                .map(|r| r.1);

            let mut writer = TableOutputWriter::new(headers, iter);

            writer.write(out)
        }
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    extern crate chrono;

    use self::chrono::{Local, TimeZone};
    use super::*;
    use std::io::Cursor;
    use std::str;
    use utils::test_utils;

    #[inline]
    fn time_to_str(value: u64) -> String {
        let dt = Local.timestamp((value / 1000) as i64, 0);
        let s = format!("{}", dt.format("%Y-%m-%d %H:%M:%S %:z"));

        s
    }

    #[test]
    fn test_sample_simple_messages() {
        let mut output = Cursor::new(Vec::new());
        let parquet = test_utils::temp_file("msg", ".parquet");
        let expected = vec![
            " field_int32  field_int64  field_float  field_double  field_string  field_boolean  field_timestamp ",
            &format!(" 1            2            3.3          4.4           \"5\"           true           {} ", time_to_str(1_238_544_000_000)),
            &format!(" 11           22           33.3         44.4          \"55\"          false          {} ", time_to_str(1_238_544_060_000)),
            ""
        ]
        .join("\n");

        let subcomand = def();
        let arg_vec = vec!["sample", parquet.path().to_str().unwrap()];
        let args = subcomand.get_matches_from_safe(arg_vec).unwrap();

        {
            let msg1 = test_utils::SimpleMessage {
                field_int32: 1,
                field_int64: 2,
                field_float: 3.3,
                field_double: 4.4,
                field_string: "5".to_string(),
                field_boolean: true,
                field_timestamp: vec![0, 0, 2_454_923],
            };
            let msg2 = test_utils::SimpleMessage {
                field_int32: 11,
                field_int64: 22,
                field_float: 33.3,
                field_double: 44.4,
                field_string: "55".to_string(),
                field_boolean: false,
                field_timestamp: vec![4_165_425_152, 13, 2_454_923],
            };

            test_utils::write_simple_messages_parquet(&parquet.path(), &[&msg1, &msg2]);

            assert_eq!(true, run(&args, &mut output).is_ok());
        }

        let vec = output.into_inner();
        let actual = str::from_utf8(&vec).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_sample_simple_messages_columns() {
        let mut output = Cursor::new(Vec::new());
        let parquet = test_utils::temp_file("msg", ".parquet");
        let path_str = parquet.path().to_str().unwrap();
        let path = parquet.path();
        let expected = vec![
            " field_boolean  field_int32 ",
            " true           1 ",
            " false          11 ",
            "",
        ]
        .join("\n");

        let subcomand = def();
        let arg_vec = vec!["sample", path_str, "-c=field_boolean", "-c=field_int32"];
        let args = subcomand.get_matches_from_safe(arg_vec).unwrap();

        let msg1 = test_utils::SimpleMessage {
            field_int32: 1,
            field_int64: 2,
            field_float: 3.3,
            field_double: 4.4,
            field_string: "5".to_string(),
            field_boolean: true,
            field_timestamp: vec![0, 0, 2_454_923],
        };
        let msg2 = test_utils::SimpleMessage {
            field_int32: 11,
            field_int64: 22,
            field_float: 33.3,
            field_double: 44.4,
            field_string: "55".to_string(),
            field_boolean: false,
            field_timestamp: vec![4_165_425_152, 13, 2_454_923],
        };

        test_utils::write_simple_messages_parquet(&path, &[&msg1, &msg2]);

        assert_eq!(true, run(&args, &mut output).is_ok());

        let vec = output.into_inner();
        let actual = str::from_utf8(&vec).unwrap();

        assert_eq!(actual, expected);
    }
}
