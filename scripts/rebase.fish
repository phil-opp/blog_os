git fetch --tags

for x in (seq 1000)
	set n (math $x + 1)
	if not git checkout post_$n 2> /dev/null
		if git checkout post_{$x}_new 2> /dev/null
			git tag -f post_$x post_{$x}_new
			git tag -d post_{$x}_new
			git push origin post_$x --force
		end
		break
	end

	if git checkout post_{$x}_new 2> /dev/null
		echo \nrebasing post_$n on top of post_{$x}_new\n--------------------------------------
		if git checkout post_{$n}_new 2> /dev/null
			echo Error: Multiple post_n_new tags!
			exit 1
		end
		if not git rebase --onto post_{$x}_new post_$x post_$n
			exit 1
		end

		git tag post_{$n}_new HEAD
		git tag -f post_$x post_{$x}_new
		git tag -d post_{$x}_new
		git push origin post_$x --force
	end
end
