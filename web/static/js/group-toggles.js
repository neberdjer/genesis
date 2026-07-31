(function () {
	var forms = [].slice.call(document.querySelectorAll("[data-group-toggles]"));
	if (!forms.length) return;

	function state(on, total, word) {
		if (total > 0 && on === total) return "all " + word;
		if (on === 0) return "none " + word;
		return on + " of " + total + " " + word;
	}

	forms.forEach(function (form) {
		var scope = form.closest(".disclosure") || document;
		var summary = scope.querySelector("[data-toggle-summary]");

		var groups = [].slice
			.call(form.querySelectorAll("[data-toggle-group]"))
			.map(function (el) {
				return {
					el: el,
					boxes: [].slice.call(el.querySelectorAll(".check input[type=checkbox]")),
					count: el.querySelector("[data-toggle-count]"),
					parent: el.querySelector("[data-toggle-parent]"),
				};
			});

		function paint(group) {
			var on = group.boxes.filter(function (b) {
				return b.checked;
			}).length;
			if (group.count) {
				group.count.textContent = state(on, group.boxes.length, group.count.dataset.toggleWord || "on");
			}
			if (group.parent) {
				group.parent.checked = group.boxes.length > 0 && on === group.boxes.length;
				group.parent.indeterminate = on > 0 && on < group.boxes.length;
			}
			return on;
		}

		function paintAll() {
			var on = 0;
			var total = 0;
			groups.forEach(function (group) {
				on += paint(group);
				total += group.boxes.length;
			});
			if (summary) {
				summary.textContent = state(on, total, summary.dataset.toggleWord || "on");
			}
		}

		form.addEventListener("change", function (e) {
			if (e.target.matches && e.target.matches("[data-toggle-parent]")) {
				var el = e.target.closest("[data-toggle-group]");
				var group = groups.find(function (g) {
					return g.el === el;
				});
				if (group) {
					group.boxes.forEach(function (b) {
						b.checked = e.target.checked;
					});
				}
			}
			paintAll();
		});

		paintAll();
	});
})();
