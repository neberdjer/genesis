(function () {
	applyTheme(storedTheme());
})();

function storedTheme() {
	try {
		return localStorage.getItem("theme");
	} catch (e) {
		return null;
	}
}

function storeTheme(mode) {
	try {
		if (mode === "system") localStorage.removeItem("theme");
		else localStorage.setItem("theme", mode);
	} catch (e) {
	}
}

function systemTheme() {
	return window.matchMedia("(prefers-color-scheme: light)").matches
		? "light"
		: "dark";
}

function currentMode() {
	var m = storedTheme();
	return m === "light" || m === "dark" ? m : "system";
}

function applyTheme(mode) {
	if (mode !== "light" && mode !== "dark") mode = "system";
	var effective = mode === "system" ? systemTheme() : mode;
	var root = document.documentElement;
	root.setAttribute("data-theme", effective);
	root.setAttribute("data-theme-mode", mode);

	var btn = document.querySelector("[data-theme-toggle]");
	if (btn) {
		btn.setAttribute("aria-label", "switch theme (currently " + mode + ")");
		btn.setAttribute("title", "theme: " + mode);
	}
}

function toggleTheme() {
	var sys = systemTheme();
	var opposite = sys === "light" ? "dark" : "light";
	var mode = currentMode();
	var next;
	if (mode === "system") next = opposite;
	else if (mode === opposite) next = sys;
	else next = "system";

	storeTheme(next);
	applyTheme(next);
}

window
	.matchMedia("(prefers-color-scheme: light)")
	.addEventListener("change", function () {
		if (currentMode() === "system") applyTheme("system");
	});

(function () {
	function closeMenus(except) {
		[].forEach.call(
			document.querySelectorAll("details.user-menu[open]"),
			function (d) {
				if (d !== except) d.removeAttribute("open");
			},
		);
	}
	document.addEventListener("click", function (e) {
		closeMenus(e.target.closest ? e.target.closest("details.user-menu[open]") : null);
	});
	document.addEventListener("keydown", function (e) {
		if (e.key === "Escape") closeMenus(null);
	});
})();
