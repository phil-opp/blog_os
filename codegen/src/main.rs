extern crate requests;
extern crate getopts;
extern crate chrono;

use chrono::Duration;
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
    use chrono::{UTC, DateTime};

    const URL: &'static str = "https://api.github.com/search/issues?q=repo:phil-opp/blog_os+type:\
                               pr+is:merged+label:relnotes";

    let mut ret = String::from("<ul>");

    let res = requests::get(URL).expect("Error while querying GitHub API");
    let data = res.json().expect("Error parsing JSON");

    for pr in data["items"].members().take(5) {
        let now = UTC::now();
        let merged_at = pr["closed_at"].as_str().unwrap().parse::<DateTime<UTC>>().unwrap();
        let ago = now - merged_at;

        let item = format!(r#"<li><a href="{}">{}</a> {}"#,
                           pr["html_url"],
                           pr["title"],
                           DateFmt(ago));
        ret.push_str(&item);
    }

    ret.push_str("</ul>");
    ret
}

struct DateFmt(Duration);

impl fmt::Display for DateFmt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, r#"<time datetime="{}">"#, self.0));

        try!(if self.0.num_minutes() == 1 {
            write!(f, "1 minute ago")
        } else if self.0.num_minutes() < 60 {
            write!(f, "{} minutes ago", self.0.num_minutes())
        } else if self.0.num_hours() == 1 {
            write!(f, "1 hour ago")
        } else if self.0.num_hours() < 24 {
            write!(f, "{} hours ago", self.0.num_hours())
        } else if self.0.num_days() == 1 {
            write!(f, "1 day ago")
        } else if self.0.num_days() < 7 {
            write!(f, "{} days ago", self.0.num_days())
        } else if self.0.num_weeks() == 1 {
            write!(f, "1 week ago")
        } else if self.0.num_weeks() < 4 {
            write!(f, "{} weeks ago", self.0.num_weeks())
        } else if self.0.num_weeks() == 4 {
            write!(f, "1 month ago")
        } else if self.0.num_days() < 365 {
            write!(f, "{} months ago", self.0.num_days() / 30)
        } else if self.0.num_days() < 365 * 2 {
            write!(f, "1 year ago")
        } else {
            write!(f, "{} years ago", self.0.num_days() / 365)
        });

        write!(f, "</datetime>")
    }
}
