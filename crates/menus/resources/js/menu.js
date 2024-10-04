var AButtonHeld = false;

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

    window.nx.footer.setAssign("A", "", () => {
        $(".is-focused").last().click();
    });
    window.nx.footer.setAssign("B", "", () => {
        window.location.href = "http://localhost/";
    });
    window.nx.footer.setAssign("X", "", () => {});
    window.nx.footer.setAssign("Y", "", () => {});

    if ($(".is-focused").length <= 0) {
        $("#list").find("button").get(0).focus();
    }
});