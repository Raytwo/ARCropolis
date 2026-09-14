
window.addEventListener("DOMContentLoaded", function() {
    var list = document.getElementById("list");

    trackButtonFocus();
    installListNavigation();

    if (isNx) {
        window.nx.footer.setAssign("A", "", function() {
            var focused = focusedButton();
            if (focused) {
                focused.click();
            }
        });
        window.nx.footer.setAssign("B", "", function() {
            window.location.href = "http://localhost/";
        });
        window.nx.footer.setAssign("X", "", function() {});
        window.nx.footer.setAssign("Y", "", function() {});
    }

    focusButton(list, 0);
});
