/* AUTO-GENERATED — do not edit by hand. Run: python site/generate_site.py */

function toggleSidebar() {
  document.getElementById('sidebar').classList.toggle('open');
}

document.getElementById('main-content').addEventListener('click', function() {
  document.getElementById('sidebar').classList.remove('open');
});

function toggleGroup(groupId) {
  var el = document.getElementById('group-' + groupId);
  if (el) el.classList.toggle('collapsed');
}

// Active link highlighting on scroll
var allAnchors = document.querySelectorAll('.section-anchor[id]');
var allNavLinks = document.querySelectorAll('.sidebar a.nav-link');

function updateActiveLink() {
  var scrollY = window.scrollY + 80;
  var current = '';
  allAnchors.forEach(function(a) { if (a.offsetTop <= scrollY) current = a.id; });
  allNavLinks.forEach(function(link) {
    var href = link.getAttribute('href');
    if (!href) return;
    var target = href.slice(1);
    link.classList.toggle('active', target === current);
  });
}

window.addEventListener('scroll', updateActiveLink, { passive: true });
updateActiveLink();
