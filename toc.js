// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="foreword.html">Foreword</a></li><li class="chapter-item expanded "><a href="ch01-00-getting-started-with-rpl.html"><strong aria-hidden="true">1.</strong> Getting Started with RPL</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="ch01-01-installation.html"><strong aria-hidden="true">1.1.</strong> Installation</a></li><li class="chapter-item expanded "><a href="ch01-02-hello-world.html"><strong aria-hidden="true">1.2.</strong> Hello World</a></li><li class="chapter-item expanded "><a href="ch01-03-why-pattern-language.html"><strong aria-hidden="true">1.3.</strong> Why We Need a Pattern Language for Code</a></li><li class="chapter-item expanded "><a href="ch01-04-cli-tour.html"><strong aria-hidden="true">1.4.</strong> Quick CLI Tour: cargo rpl</a></li></ol></li><li class="chapter-item expanded "><a href="ch02-00-language-fundamentals.html"><strong aria-hidden="true">2.</strong> Language Fundamentals</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="ch02-01-syntax-overview.html"><strong aria-hidden="true">2.1.</strong> Syntax Overview</a></li><li class="chapter-item expanded "><a href="ch02-02-meta-variables-placeholders.html"><strong aria-hidden="true">2.2.</strong> Meta-Variabl es &amp; Placeholders</a></li><li class="chapter-item expanded "><a href="ch02-03-pattern-blocks.html"><strong aria-hidden="true">2.3.</strong> Pattern Blocks</a></li><li class="chapter-item expanded "><a href="ch02-04-constraints-predicates.html"><strong aria-hidden="true">2.4.</strong> Constraints &amp; Predicates</a></li><li class="chapter-item expanded "><a href="ch02-05-match-output.html"><strong aria-hidden="true">2.5.</strong> Match Output &amp; Diagnostics</a></li></ol></li><li class="chapter-item expanded "><a href="ch03-00-semantics-model.html"><strong aria-hidden="true">3.</strong> Semantics Model</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="ch03-01-mir-basics.html"><strong aria-hidden="true">3.1.</strong> MIR Basics and Integration</a></li><li class="chapter-item expanded "><a href="ch03-02-cfg-ddg.html"><strong aria-hidden="true">3.2.</strong> Control-Flow &amp; Data-Dependence Graphs</a></li><li class="chapter-item expanded "><a href="ch03-03-semantic-equivalence.html"><strong aria-hidden="true">3.3.</strong> Semantic Equivalence Checking</a></li><li class="chapter-item expanded "><a href="ch03-04-inline-mir.html"><strong aria-hidden="true">3.4.</strong> Inline MIR vs Traditional MIR</a></li></ol></li><li class="chapter-item expanded "><a href="ch04-00-case-studies.html"><strong aria-hidden="true">4.</strong> Case Studies</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="ch04-01-memory-safety.html"><strong aria-hidden="true">4.1.</strong> Memory-Safety CVEs</a></li><li class="chapter-item expanded "><a href="ch04-02-undefined-behavior.html"><strong aria-hidden="true">4.2.</strong> Undefined Behavior</a></li><li class="chapter-item expanded "><a href="ch04-03-open-source.html"><strong aria-hidden="true">4.3.</strong> Open-Source Projects</a></li></ol></li><li class="chapter-item expanded "><a href="appendix-00-appendices.html"><strong aria-hidden="true">5.</strong> Appendices</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="appendix-01-syntax.html"><strong aria-hidden="true">5.1.</strong> Complete Syntax Reference</a></li><li class="chapter-item expanded "><a href="appendix-02-error-codes.html"><strong aria-hidden="true">5.2.</strong> Error Codes &amp; Remedies</a></li><li class="chapter-item expanded "><a href="appendix-03-patterns.html"><strong aria-hidden="true">5.3.</strong> Patterns</a></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
