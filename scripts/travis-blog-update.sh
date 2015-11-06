#!/bin/sh

# License: CC0 1.0 Universal
# https://creativecommons.org/publicdomain/zero/1.0/legalcode

set -e

[ "$TRAVIS_BRANCH" = master ]

[ "$TRAVIS_PULL_REQUEST" = false ]

eval SSH_KEY_TRAVIS_ID=aaae456e27e9
eval key=\$encrypted_${SSH_KEY_TRAVIS_ID}_key
eval iv=\$encrypted_${SSH_KEY_TRAVIS_ID}_iv

mkdir -p ~/.ssh
openssl aes-256-cbc -K $key -iv $iv -in scripts/travis-blog_os.enc -out ~/.ssh/id_rsa -d
chmod 600 ~/.ssh/id_rsa

git clone --branch gh-pages git@github.com:$TRAVIS_REPO_SLUG deploy_blog

cd deploy_blog
git config user.name "blog update bot"
git config user.email "nobody@example.com"
cp -r ../posts _posts
cp ../pages/* ./
git add _posts _pages
git commit -qm "update blog to $TRAVIS_COMMIT"
git push -q origin gh-pages
