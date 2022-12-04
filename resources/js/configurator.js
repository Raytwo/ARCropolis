var selected_workspace = 0;

window.addEventListener("DOMContentLoaded", (e) => {
    var buttons = document.querySelectorAll('button');

    [].forEach.call(buttons, function(btn) {
        btn.addEventListener("focus", () => {
            btn.classList.add("is-focused");
        });

        btn.addEventListener("focusout", () => {
            btn.classList.remove("is-focused");
        });
    });

    window.addEventListener('keydown', function(e) {
        if (e.keyCode == UP) {
            var target = document.querySelector(".is-focused").previousElementSibling;
            if (target != undefined) {
                getCurrentActiveContainer()[0].scrollTop = target.offsetTop + 50;
                target.focus();
            }
        } else if (e.keyCode == DOWN) {
            var target = document.querySelector(".is-focused").nextElementSibling;
            if (target != undefined) {
                getCurrentActiveContainer()[0].scrollTop = target.offsetTop - 50;
                target.focus();
            }
        }
    });

    if ($(".is-focused").length <= 0) {
        getCurrentActiveContainer().find("button").get(0).focus();
    }

    window.nx.addEventListener("message", function(e) {
        document.getElementById(e.data).classList.toggle("hidden");
    });

    // Code to handle this session wasn't made to detect a closure by button
    window.nx.footer.setAssign("B", "", () => {
        if (getCurrentActiveContainer().attr("id") != "workspaces") {
            changeDivFromTo('logging', 'workspaces', `0`);
        } else {
            submit(`exit`, `true`);
        }
    });

    window.nx.sendMessage("loaded");
});

function getCurrentActiveContainer() {
    if ($("#workspaces").is(":visible")) {
        return $("#workspaces");
    } else if ($("#logging").is(":visible")) {
        return $("#logging");
    }
}

function changeDivFromTo(from, to, workspace) {
    current_page = to;
    if (from == "workspaces") {
        selected_workspace = workspace;
    }

    $(`#${from}`).fadeOut(200);
    $(`#${from}`).promise().done(function() {
        $(`#${to}`).fadeIn(200);
        if (to == "workspaces") {
            $(`#${to}`).find($("button")[parseInt(selected_workspace)]).get(0).focus();
        } else {
            $(`#${to}`).find("button").get(0).focus();
        }
        // document.getElementById("test").innerHTML = $(`#${to}`).find("button").length;
    });
}

function submit(cat, type) {
    var result = {
        category: cat,
        value: type,
    };
    window.nx.sendMessage(JSON.stringify(result));

    //var result = `${type}|${selected_workspace}`;
    //location.href = `http://localhost/${result}`;
}