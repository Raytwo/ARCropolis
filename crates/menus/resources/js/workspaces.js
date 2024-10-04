const WORKSPACES_CONTROL = "&#xe000 Set Active &nbsp; &#xe002 Duplicate Workspace &nbsp; &#xe003 Show Options";
const WORKSPACE_CONTROL = "&#xe000 Select Option";

var workspaces = [];
var selected_workspace = 0;
var active_workspace = "";

window.addEventListener("DOMContentLoaded", (e) => {
    if (!isNx) {
        for (var i = 0; i < 10; i++) {
            workspaces.push(`Workspace #${i + 1}`);
            setupWorkspaces();
        }

    } else {

        $.ajax({
            dataType: "json",
            url: "workspaces.json",
            success: (data) => {
                workspaces = data["workspaces"];
                active_workspace = data["active_workspace"];
                setupWorkspaces();
            }
        });

        // window.nx.sendMessage(JSON.stringify({
        //     "WriteItDown": {
        //         "text": JSON.stringify(Object.getOwnPropertyNames(window.nx).filter(function(p) {
        //             return typeof window.nx[p] === 'function';
        //         }).map(x => ({
        //             "function": x,
        //             "parametersCount": window.nx[x].length
        //         })))
        //     }
        // }));

        window.nx.footer.setAssign("A", "", () => {
            if ($(".is-focused").length <= 0) {
                $("button:visible").get(0).focus();
            } else {
                $(".is-focused").get(0).click();
            }
        });
        window.nx.footer.setAssign("B", "", () => {
            goBack();
        });
        window.nx.footer.setAssign("X", "", () => {
            if (getCurrentActiveContainer().attr('id') == "workspaces") {
                selected_workspace = parseInt($(".is-focused").attr('data-id'));
                if (selected_workspace == undefined || isNaN(selected_workspace) || selected_workspace == null) {
                    return;
                }

                if (duplicateWorkspace()) {
                    changeDivFromTo('workspaces', 'workspaces');
                }
            }
        });
        window.nx.footer.setAssign("Y", "", () => {
            if (getCurrentActiveContainer().attr('id') == "workspaces") {
                selected_workspace = parseInt($(".is-focused").attr('data-id'));
                if (selected_workspace == undefined || isNaN(selected_workspace) || selected_workspace == null) {
                    return;
                }
                showWorkspace(selected_workspace);
            }
        });
    }

    // Listen to the keydown event and prevent the default
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
});

function goBack() {
    if (getCurrentActiveContainer().attr('id') == "workspaceOption") {
        changeDivFromTo('workspaceOption', 'workspaces', selected_workspace);
    } else {
        exit();
    }
}

function exit() {
    window.nx.sendMessage(JSON.stringify("ClosureRequest"));
    window.location.href = "http://localhost/quit";
}

function getCurrentActiveContainer() {
    if ($("#workspaces").is(":visible")) {
        return $("#workspaces");
    } else if ($("#workspaceOption").is(":visible")) {
        return $("#workspaceOption");
    }
}

function changeDivFromTo(from, to) {
    if (to == "workspaceOption") {
        $("#workspaceArrow").show();
        $("#workspace").html(workspaces[selected_workspace]);
        $("#workspace").show();
        $("#message").html(WORKSPACE_CONTROL);
    } else if (to == "workspaces") {
        $("#workspaceArrow").hide();
        $("#workspace").hide();
        $("#message").html(WORKSPACES_CONTROL);
    }

    $(`#${from}`).fadeOut(200);
    $(`#${from}`).promise().done(function() {
        $(`#${to}`).fadeIn(200);
        if (to == "workspaces") {
            setupWorkspaces();
            $(`#${to}`).find("button:visible").get(parseInt(selected_workspace)).focus();
        } else {
            $(`#${to}`).find("button:visible").get(0).focus();
        }
    });
}

function setupWorkspaces() {
    workspaces.sort(function(a, b) {
        return a.localeCompare(b);
    });
    var htmlText = "";
    for (var i = 0; i < workspaces.length; i++) {
        var display = workspaces[i] == active_workspace ? 'block' : 'none';
        htmlText += `<button onclick="selected_workspace = ${i}; setActive();" data-id='${i}' class="flex-item">
        <div class="icon-background"><img class="abstract-icon is-appear" style="display: ${display}" src="check.svg" /></div>
        <div class="item-container">
            <h2>${workspaces[i]}</h2>
        </div>
    </button>`;
    }

    document.getElementById("workspacesContainer").innerHTML = htmlText + `
    <button onclick="createWorkspace()" class="flex-item">
        <div class="icon-background"></div>
        <div class="item-container">
            <h2>Create Workspace</h2>
        </div>
    </button>
    `;

    var buttons = document.querySelectorAll('button');

    [].forEach.call(buttons, function(btn) {
        btn.addEventListener("focus", () => {
            btn.classList.add("is-focused");
        });

        btn.addEventListener("focusout", () => {
            btn.classList.remove("is-focused");
        });
    });

    $("#workspacesContainer>button:first-child").get(0).focus();
}

function showWorkspace(idx) {
    selected_workspace = idx;
    if (workspaces[selected_workspace] == "Default") {
        $("#removeWorkspace").hide();
        $("#renameWorkspace").hide();
    } else {
        $("#removeWorkspace").show();
        $("#renameWorkspace").show();
    }

    if (workspaces[selected_workspace] == active_workspace) {
        $("#is-active img").show();
    } else {
        $("#is-active img").hide();
    }

    changeDivFromTo('workspaces', 'workspaceOption', idx);
}

function setActive() {
    active_workspace = workspaces[selected_workspace];
    if (getCurrentActiveContainer().attr('id') == "workspaceOption") {
        $("#is-active img").show();
    } else {
        $("button:visible img").hide();
        $(`button:visible[data-id='${selected_workspace}'] img`).show();
    }

    if (isNx) {
        window.nx.sendMessage(JSON.stringify({
            "SetActive": {
                "name": active_workspace
            }
        }));
    }
}

function renameWorkspace() {
    if (workspaces[selected_workspace] == "Default") { return; }

    var res = prompt("Rename workspace", workspaces[selected_workspace]);
    if (res == null || res == undefined) { return; }

    if (workspaces.includes(res)) {
        alert("Workspace with that name already exists!");
        return;
    }

    sourceName = workspaces[selected_workspace];
    targetName = res;

    workspaces[selected_workspace] = targetName;

    $("#workspace").html(workspaces[selected_workspace]);

    if (isNx) {
        window.nx.sendMessage(JSON.stringify({
            "Rename": {
                "source_name": sourceName,
                "target_name": targetName,
            }
        }));

        if (active_workspace == sourceName) {
            setActive();
        }
    }
}

function duplicateWorkspace() {
    var res = prompt("Name for duplicated workspace", workspaces[selected_workspace]);
    if (res == null || res == undefined) { return false; }

    if (workspaces.includes(res)) {
        alert("Workspace with that name already exists!");
        return false;
    }

    sourceName = workspaces[selected_workspace];
    targetName = res;

    workspaces.push(targetName);

    if (isNx) {
        window.nx.sendMessage(JSON.stringify({
            "Duplicate": {
                "source_name": sourceName,
                "target_name": targetName,
            }
        }));
    }

    return true;
}

function removeWorkspace() {
    if (workspaces[selected_workspace] == "Default") { return; }

    if (confirm(`Do you really want to delete workspace ${workspaces[selected_workspace]}?`)) {
        if (confirm(`Are you really sure you want to delete workspace ${workspaces[selected_workspace]}?`)) {
            if (isNx) {
                window.nx.sendMessage(JSON.stringify({
                    "Remove": {
                        "name": workspaces[selected_workspace],
                    }
                }));
            }

            workspaces.splice(selected_workspace, 1);
            changeDivFromTo('workspaceOption', 'workspaces', 0);
        }
    }
}

function createWorkspace() {
    var res = prompt("Enter new workspace name");
    if (res == null || res == undefined) { return; }

    if (workspaces.includes(res)) {
        alert("Workspace with that name already exists!");
        return;
    }

    workspaces.push(res);
    // send nx message
    if (isNx) {
        window.nx.sendMessage(JSON.stringify({
            "Create": {
                "name": res
            }
        }));
    }
    selected_workspace = workspaces.length - 1;
    changeDivFromTo('workspaces', 'workspaces');
}

function editWorkspace() {
    if (isNx) {
        window.nx.sendMessage(JSON.stringify({
            "Edit": {
                "name": workspaces[selected_workspace]
            }
        }));
        window.location.href = "http://localhost/quit";
    }
}