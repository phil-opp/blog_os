set original (git rev-parse --abbrev-ref HEAD)
set list
for x in (seq 99)
    set current (printf "post-%02i" $x)
	if not git checkout $current --quiet
        break
    end
    set list $list $current
end
git push origin $list
git checkout $original --quiet
