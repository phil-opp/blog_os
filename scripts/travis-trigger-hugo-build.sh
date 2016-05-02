#!/bin/sh

set -e

[ "$TRAVIS_BRANCH" = master ]
[ "$TRAVIS_PULL_REQUEST" = false ]

body='{
"request": {
  "branch":"hugo",
  "config": {
    "env": {
      "matrix": ["UPDATE_COMMIT=$TRAVIS_COMMIT"]
    }
  }
}}'

curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -H "Travis-API-Version: 3" \
  -H "Authorization: token $TRAVIS_TOKEN" \
  -d "$body" \
  https://api.travis-ci.org/repo/phil-opp%2Fblog_os/requests
