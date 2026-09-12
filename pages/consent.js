/* Basic consent mode: no Google request until optional consent is granted. */
(() => {
  "use strict";
  const KEY = "svb-consent-v1";
  const TTL = 180 * 24 * 60 * 60 * 1000;
  const denied = {
    analytics: false,
    advertising: false,
    personalization: false,
  };
  let loaded = false;
  let current = null;
  window.dataLayer = window.dataLayer || [];
  function gtag() {
    window.dataLayer.push(arguments);
  }
  function signals(choice) {
    return {
      analytics_storage: choice.analytics ? "granted" : "denied",
      ad_storage: choice.advertising ? "granted" : "denied",
      ad_user_data: choice.advertising ? "granted" : "denied",
      ad_personalization:
        choice.advertising && choice.personalization ? "granted" : "denied",
      functionality_storage: "denied",
      personalization_storage: "denied",
      security_storage: "granted",
    };
  }
  gtag("consent", "default", signals(denied));
  gtag("set", "ads_data_redaction", true);
  gtag("set", "url_passthrough", false);
  function read() {
    try {
      const value = JSON.parse(localStorage.getItem(KEY));
      if (
        !value ||
        value.version !== 1 ||
        !Number.isFinite(value.time) ||
        Date.now() - value.time >= TTL ||
        value.time > Date.now() ||
        ["analytics", "advertising", "personalization"].some(
          (key) => typeof value[key] !== "boolean",
        )
      )
        return null;
      return {
        analytics: value.analytics,
        advertising: value.advertising,
        personalization: value.advertising && value.personalization,
      };
    } catch {
      return null;
    }
  }
  function apply(choice) {
    gtag("consent", "update", signals(choice));
    window.dataLayer.push({
      event: "svb_consent_update",
      svb_analytics: choice.analytics,
      svb_advertising: choice.advertising,
      svb_personalization: choice.advertising && choice.personalization,
    });
    if (!loaded && (choice.analytics || choice.advertising)) {
      loaded = true;
      window.dataLayer.push({ "gtm.start": Date.now(), event: "gtm.js" });
      const script = document.createElement("script");
      script.async = true;
      script.src = "https://www.googletagmanager.com/gtm.js?id=GTM-P7RPRQBZ";
      document.head.appendChild(script);
    }
  }
  function clearCookies(choice) {
    const patterns = [];
    if (!choice.analytics) patterns.push(/^_ga(?:_|$)/, /^_gid$/, /^_gat/);
    if (!choice.advertising) patterns.push(/^_gcl_/);
    const host = location.hostname.split(".");
    const domains = [""];
    for (let i = 0; i < host.length - 1; i++)
      domains.push(host.slice(i).join("."));
    const parts = location.pathname.split("/");
    const paths = new Set(["/"]);
    for (let i = 1; i < parts.length; i++)
      paths.add(parts.slice(0, i).join("/") || "/");
    for (const cookie of document.cookie.split(";")) {
      const name = cookie.split("=")[0].trim();
      if (!patterns.some((pattern) => pattern.test(name))) continue;
      for (const domain of domains)
        for (const path of paths) {
          document.cookie = `${name}=; Max-Age=0; Path=${path}; SameSite=Lax${domain ? `; Domain=${domain}` : ""}`;
        }
    }
  }
  const banner = document.getElementById("consent-banner");
  const advanced = document.getElementById("consent-advanced");
  const analytics = document.getElementById("consent-analytics");
  const advertising = document.getElementById("consent-advertising");
  const personalization = document.getElementById("consent-personalization");
  const toggle = document.getElementById("consent-customize");
  const status = document.getElementById("consent-status");
  function sync() {
    personalization.disabled = !advertising.checked;
    if (!advertising.checked) personalization.checked = false;
  }
  function show() {
    analytics.checked = current?.analytics || false;
    advertising.checked = current?.advertising || false;
    personalization.checked = current?.personalization || false;
    sync();
    banner.hidden = false;
  }
  function save(choice) {
    try {
      localStorage.setItem(
        KEY,
        JSON.stringify({ ...choice, version: 1, time: Date.now() }),
      );
    } catch {
      status.textContent = loaded
        ? "Your browser could not save the change. Your previous choices are still active. Clear this site’s data in your browser settings to withdraw consent."
        : "Your browser could not save this choice. Optional tracking remains off. Allow site storage to save your preferences.";
      return;
    }
    const revoked =
      loaded &&
      ["analytics", "advertising", "personalization"].some(
        (key) => current?.[key] && !choice[key],
      );
    clearCookies(choice);
    current = choice;
    banner.hidden = true;
    if (revoked) {
      // Reload before any more events so previously loaded tags cannot keep running.
      location.reload();
      return;
    }
    apply(choice);
    document.getElementById("consent-open").focus({ preventScroll: true });
  }
  document
    .getElementById("consent-accept")
    .addEventListener("click", () =>
      save({ analytics: true, advertising: true, personalization: true }),
    );
  document
    .getElementById("consent-reject")
    .addEventListener("click", () => save({ ...denied }));
  document.getElementById("consent-save").addEventListener("click", () =>
    save({
      analytics: analytics.checked,
      advertising: advertising.checked,
      personalization: advertising.checked && personalization.checked,
    }),
  );
  advertising.addEventListener("change", sync);
  toggle.addEventListener("click", () => {
    advanced.hidden = !advanced.hidden;
    toggle.setAttribute("aria-expanded", String(!advanced.hidden));
  });
  document.getElementById("consent-open").addEventListener("click", () => {
    show();
    advanced.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
    toggle.focus();
  });
  window.addEventListener("storage", (event) => {
    if (event.key === KEY || event.key === null) location.reload();
  });
  window.addEventListener("pageshow", (event) => {
    if (event.persisted) location.reload();
  });
  current = read();
  if (current) {
    clearCookies(current);
    apply(current);
  } else {
    clearCookies(denied);
    show();
  }
})();
