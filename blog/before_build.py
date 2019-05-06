#!/usr/bin/env python
# -*- coding: utf-8 -*-

import io
from github import Github
from datetime import datetime, timedelta

g = Github()

one_month_ago = datetime.now() - timedelta(days=32)

def filter_date(issue):
    return issue.closed_at > one_month_ago

with io.open("templates/recent-updates.html", 'w', encoding='utf8') as recent_updates:
    recent_updates.truncate()

    recent_updates.write(u"<ul>\n")

    issues = g.search_issues("is:merged", repo="phil-opp/blog_os", type="pr", label="relnotes")[:10]

    for pr in filter(filter_date, issues):
        link = '<a href="' + pr.html_url + '">' + pr.title + "</a> "
        iso_date = pr.closed_at.isoformat()
        readable_date = pr.closed_at.strftime("%b&nbsp;%d")
        datetime = '<time datetime="' + iso_date + '">' + readable_date + '</time>'
        recent_updates.write(u"  <li>" + link + datetime + "</li>\n")

    recent_updates.write(u"</ul>")
