#!/usr/bin/env python
# -*- coding: utf-8 -*-

import io
import urllib2
import datetime
from github import Github

g = Github()

one_month_ago = datetime.datetime.now() - datetime.timedelta(days=32)

def filter_date(issue):
    return issue.closed_at > one_month_ago

def format_number(number):
    if number > 1000:
        return u"{:.1f}k".format(float(number) / 1000)
    else:
        return u"{}".format(number)

with io.open("templates/auto/recent-updates.html", 'w', encoding='utf8') as recent_updates:
    recent_updates.truncate()

    relnotes_issues = g.search_issues("is:merged", repo="phil-opp/blog_os", type="pr", label="relnotes")[:100]
    recent_relnotes_issues = filter(filter_date, relnotes_issues)

    if len(recent_relnotes_issues) == 0:
        recent_updates.write(u"No notable updates recently.")
    else:
        recent_updates.write(u"<ul>\n")

        for pr in sorted(recent_relnotes_issues, key=lambda issue: issue.closed_at, reverse=True):
            link = '<a href="' + pr.html_url + '">' + pr.title + "</a> "
            iso_date = pr.closed_at.isoformat()
            readable_date = pr.closed_at.strftime("%b&nbsp;%d")
            datetime_str = '<time datetime="' + iso_date + '">' + readable_date + '</time>'
            recent_updates.write(u"  <li>" + link + datetime_str + "</li>\n")

        recent_updates.write(u"</ul>")

repo = g.get_repo("phil-opp/blog_os")

with io.open("templates/auto/stars.html", 'w', encoding='utf8') as stars:
    stars.truncate()
    stars.write(format_number(repo.stargazers_count))

with io.open("templates/auto/forks.html", 'w', encoding='utf8') as forks:
    forks.truncate()
    forks.write(format_number(repo.forks_count))


# query "This week in Rust OSDev posts"

lines = []
year = 2020
month = 4
while True:
    url = "https://rust-osdev.com/this-month/" + str(year) + "-" + str(month).zfill(2) + "/"
    try:
        urllib2.urlopen(url)
    except urllib2.HTTPError as e:
        break

    month_str = datetime.date(1900, month, 1).strftime('%B')

    link = '<a href="' + url + '">This Month in Rust OSDev (' + month_str + " " + str(year) + ")</a> "
    lines.append(u"  <li>" + link + "</li>\n")

    month = month + 1
    if month > 12:
        month = 0
        year = year + 1

lines.reverse()

with io.open("templates/auto/status-updates.html", 'w', encoding='utf8') as status_updates:
    status_updates.truncate()

    for line in lines:
        status_updates.write("<b>" + line + "</b>")

with io.open("templates/auto/status-updates-truncated.html", 'w', encoding='utf8') as status_updates:
    status_updates.truncate()

    for index, line in enumerate(lines):
        if index == 5:
            break
        status_updates.write("<b>" + line + "</b>")
