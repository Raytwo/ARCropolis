var workspaces = [];
var selected_workspace = 0;
var active_workspace = "";
var AButtonHeld = false;
var BButtonHeld = false;

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

        window.nx.footer.setAssign("B", "", () => {});
        window.nx.footer.setAssign("X", "", () => {});
    }

    // Listen to the keydown event and prevent the default
    window.addEventListener('keydown', function(e) {
        e.preventDefault();
    });

    // Listen to the gamepadconnected event
    window.addEventListener("gamepadconnected", function(e) {
        if ($(".is-focused").length <= 0) {
            getCurrentActiveContainer().find("button").get(0).focus();
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
    } else if (to == "workspaces") {
        $("#workspaceArrow").hide();
        $("#workspace").hide();
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

function checkGamepad(index, gamepad) {
    //#region UI Input Check

    var axisX = gamepad.axes[0];
    var axisY = gamepad.axes[1];

    // Check A button
    if (gamepad.buttons[1].pressed) {
        if (!AButtonHeld[index]) {
            AButtonHeld[index] = true;
            if ($(".is-focused").length <= 0) {
                $("button:visible").get(0).focus();
            } else {
                $(".is-focused").last().click();
            }
        }
    } else {
        AButtonHeld[index] = false;
    }

    // Check B Button
    if (gamepad.buttons[0].pressed) {
        if (!BButtonHeld) {
            goBack();
            BButtonHeld = true;
        }
    } else {
        BButtonHeld = false;
    }

    var target = undefined;
    var offset = undefined;

    // Check if D-pad Left pressed or Left Stick X Axis less than -0.7
    if (gamepad.buttons[14].pressed || axisX < -0.7) {
        // Go up by 6 elements
        var slice_index = 6;
        target = $(".is-focused").prevAll(":visible").slice(0, slice_index).last();
        while (target.length <= 0 && slice_index != 0) {
            slice_index -= 1;
            target = $(".is-focused").prevAll(":visible").slice(0, slice_index).last();
        }

        // If that doesn't exist, then dip
        if (target.length <= 0) {
            return;
        }

        offset = $(getCurrentActiveContainer()).scrollTop() + target.position().top - 50;
    }
    // Check if D-pad Up pressed or Y-Axis
    else if (gamepad.buttons[12].pressed || axisY < -0.7) {
        // Get the mod above the current focused one
        target = $(".is-focused").prev();

        while (target.length > 0 && target.is(':hidden')) {
            target = target.prev();
        }

        // If that doesn't exist, then dip
        if (target.length <= 0) {
            return;
        }

        offset = $(getCurrentActiveContainer()).scrollTop() + target.position().top - 50;
    }
    // Check if D-pad Right pressed or X Axis > 0.7
    else if (gamepad.buttons[15].pressed || axisX > 0.7) {
        // Go up down 6 elements
        var slice_index = 6;
        target = $(".is-focused").nextAll(":visible").slice(0, slice_index).last();

        while (target.length <= 0 && slice_index != 0) {
            slice_index -= 1;
            target = $(".is-focused").nextAll(":visible").slice(0, slice_index).last();
        }

        // If that doesn't exist, then dip
        if (target.length <= 0) {
            return;
        }

        offset = ($(getCurrentActiveContainer()).scrollTop()) + (target.height() * 2);
    }
    // Check if D-pad Down pressed or Y Axis > 0.7
    else if (gamepad.buttons[13].pressed || axisY > 0.7) {
        // Get the next mod that will be focused on
        target = $(".is-focused").next();

        while (target.length > 0 && target.is(':hidden')) {
            target = target.next();
        }

        console.log(target);
        // If there is none after that, then just return
        if (target.length <= 0) {
            return;
        }

        offset = ($(getCurrentActiveContainer()).scrollTop()) + (target.height() * 2);
    };

    if (target != undefined) {
        scroll(target, offset, getCurrentActiveContainer());
    }

    //#endregion
}

function setupWorkspaces() {
    workspaces.sort(function(a, b) {
        return a.localeCompare(b);
    });
    var htmlText = "";
    for (var i = 0; i < workspaces.length; i++) {
        var img = workspaces[i] == active_workspace ? `<img class="abstract-icon is-appear" src="check.svg" />` : "";
        htmlText += `<button onclick="showWorkspace(${i})" class="flex-item">
        <div class="icon-background">${img}</div>
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
    $("#is-active img").show();
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
    if (res == null || res == undefined) { return; }

    if (workspaces.includes(res)) {
        alert("Workspace with that name already exists!");
        return;
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