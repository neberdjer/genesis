(function () {
	var input = document.getElementById("cmd-search");
	if (!input) return;

	var empty = document.getElementById("cmd-empty");

	var groups = [].map.call(
		document.querySelectorAll(".cmd-group"),
		function (group) {
			var cmds = [].map.call(group.querySelectorAll(".cmd"), function (el) {
				return { el: el, text: el.textContent.toLowerCase() };
			});
			return { el: group, cmds: cmds };
		},
	);

	var raf = 0;
	function filter() {
		var q = input.value.trim().toLowerCase();
		var total = 0;

		groups.forEach(function (group) {
			var shown = 0;
			group.cmds.forEach(function (cmd) {
				var match = cmd.text.indexOf(q) !== -1;
				cmd.el.style.display = match ? "" : "none";
				if (match) shown++;
			});
			group.el.style.display = shown > 0 ? "" : "none";
			total += shown;
		});

		if (empty) empty.style.display = total === 0 ? "" : "none";
	}

	input.addEventListener("input", function () {
		if (raf) return;
		raf = requestAnimationFrame(function () {
			raf = 0;
			filter();
		});
	});
})();
