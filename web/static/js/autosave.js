(function () {
	[].forEach.call(document.querySelectorAll("form.autosave"), function (form) {
		var status = form.querySelector(".save-status");
		var timer = 0;
		var inflight = false;
		var dirty = false;

		form.addEventListener("change", function () {
			setStatus("saving", null);
			if (inflight) {
				dirty = true;
				return;
			}
			clearTimeout(timer);
			timer = setTimeout(run, 400);
		});

		function run() {
			inflight = true;
			dirty = false;
			var body = new URLSearchParams(new FormData(form)).toString();
			fetch(form.action, {
				method: "POST",
				headers: { "Content-Type": "application/x-www-form-urlencoded" },
				body: body,
			})
				.then(function (r) {
					if (r.ok) setStatus("saved", true);
					else if (r.status === 429) setStatus("too fast, wait a moment", false);
					else setStatus("couldn't save", false);
				})
				.catch(function () {
					setStatus("couldn't save", false);
				})
				.then(function () {
					inflight = false;
					if (dirty) {
						clearTimeout(timer);
						timer = setTimeout(run, 400);
					}
				});
		}

		function setStatus(text, ok) {
			if (!status) return;
			status.textContent = text;
			status.className = "save-status" + (ok === true ? " ok" : ok === false ? " err" : "");
		}
	});
})();
