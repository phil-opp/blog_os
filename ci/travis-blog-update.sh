#!/bin/sh

set -e

# update blog if current branch is `hugo`
[ "$TRAVIS_BRANCH" = hugo ]
[ "$TRAVIS_PULL_REQUEST" = false ]

# decrypt ssh key
eval SSH_KEY_TRAVIS_ID=aaae456e27e9
eval key=\$encrypted_${SSH_KEY_TRAVIS_ID}_key
eval iv=\$encrypted_${SSH_KEY_TRAVIS_ID}_iv

mkdir -p ~/.ssh
openssl aes-256-cbc -K $key -iv $iv -in ci/travis-blog_os.enc -out ~/.ssh/id_rsa -d
chmod 600 ~/.ssh/id_rsa

# clone gh-pages to `deploy_blog`
git clone --branch gh-pages git@github.com:$TRAVIS_REPO_SLUG deploy_blog
cd deploy_blog

# set git user/email
git config user.name "travis-update-bot"
git config user.email "travis-update-bot@phil-opp.com"

# update blog
rm -r *
cp -r ../public/. .
rm atom.xml # remove feed that includes all content types
mv post/atom.xml . # use post feed as main feed
rm -r post post.html page page.html # remove per-category pages/feeds

# commit
git add --all .
git commit -qm "Update blog to $TRAVIS_COMMIT"

# push changes
git push -q origin gh-pages
