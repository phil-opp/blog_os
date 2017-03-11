extern crate requests;
extern crate getopts;
extern crate chrono;

use chrono::{DateTime, UTC};
use std::fmt;

fn main() {
    use std::env;
    use std::fs::File;
    use std::io::Write;

    let args: Vec<String> = env::args().collect();
    let mut opts = getopts::Options::new();

    opts.optopt("o", "", "set output file name", "NAME");
    let matches = opts.parse(&args[1..]).unwrap();
    let output = matches.opt_str("o");

    let pr_list = pr_list();

    match output {
        None => println!("{:?}", pr_list),
        Some(output) => {
            let mut file = File::create(output).expect("error while opening/creating output file");
            file.write_all(pr_list.as_bytes()).expect("error while writing to output file");
        }
    }
}

fn pr_list() -> String {
    const URL: &'static str = "https://api.github.com/search/issues?q=repo:phil-opp/blog_os+type:\
                               pr+is:merged+label:relnotes";

    let mut ret = String::from("<ul>");

    let res = requests::get(URL).expect("Error while querying GitHub API");
    let data = res.json().expect("Error parsing JSON");

    for pr in data["items"].members().take(5) {
        let merged_at = pr["closed_at"].as_str().unwrap().parse::<DateTime<UTC>>().unwrap();
        let item = format!("<li><a href='{}'>{}</a> {}",
                           pr["html_url"],
                           pr["title"],
                           DateFmt(merged_at));
        ret.push_str(&item);
    }

    ret.push_str("</ul>");
    ret
}

struct DateFmt(DateTime<UTC>);

impl fmt::Display for DateFmt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               r#"<time datetime="{}">{}</datetime>"#,
               self.0,
               self.0.format("%b\u{a0}%d"))
    }
}
