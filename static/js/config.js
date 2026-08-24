// Overrides themes/neovim-theme/static/js/config.js.
//
// Zola copies the site's static/ over the theme's, so this file replaces the
// theme's copy wholesale — `keys` and `commands` are declared once, here.
// The only substantive change is that navigation goes through window.BASE_URL
// (set in templates/base.html) so it survives a sub-path deploy.

function exec_config() {
  const config = JSON.parse(localStorage.getItem("config"));

  Object.keys(config).map((key) => {
    const value = config[key];
    switch (key) {
      case "mouse":
        const html = document.getRootNode().documentElement;
        html.style = value ? "" : "cursor:none;pointer-events:none;";
        break;

      default:
    }
  });
}

// get_url(path="/") does not guarantee a trailing slash, so normalise here
// rather than depending on how the template rendered it.
const base = () => {
  const b = window.BASE_URL || "/";
  return b.endsWith("/") ? b : b + "/";
};

const keys = {
  normal: {
    escape: () => {
      document.getElementById("setter").focus();
      document.getElementById("setter").value = "";
    },

    enter: (event, element, { is_prompt }) => {
      if (is_prompt) {
        command();
      } else {
        new_tab(element, true);
      }
    },

    j: (event, element, { is_viewer, is_page, is_prompt }) => {
      if (is_viewer && is_page) {
        element.scrollBy(0, 30);
      } else if (!is_prompt) {
        next_file(-1, element);
      }
    },

    k: (event, element, { is_viewer, is_page, is_prompt }) => {
      if (is_viewer && is_page) {
        element.scrollBy(0, -30);
      } else if (!is_prompt) {
        next_file(1, element);
      }
    },

    l: (event, element, { is_prompt }) => {
      if (!is_prompt) element.scrollBy(30, 0);
    },

    h: (event, element, { is_prompt }) => {
      if (!is_prompt) element.scrollBy(-30, 0);
    },
  },

  shortcut: {
    l: () => {
      document.getElementById("viewer").focus();
      localStorage.setItem("focused", "viewer");
    },

    h: () => {
      document.getElementById("files").focus();
      localStorage.setItem("focused", "files");
    },

    t: (event, element) => {
      new_tab(element);
    },

    q: () => {
      del_tab();
    },

    tab: () => {
      next_tab();
    },
  },
};

const commands = {
  help: () => {
    window.location.href = base() + "readme/";
  },

  home: () => {
    window.location.href = base();
  },

  // `:q` quits the page you are on rather than leaving for a hardcoded
  // third-party site, which is what the theme shipped.
  q: () => {
    window.history.back();
  },

  set: (args, setter) => {
    const success = set(args);
    setter.value = JSON.stringify(success);
  },
};
