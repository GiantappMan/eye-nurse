import { createApp } from "vue";
import { createPinia } from "pinia";
import { languages } from "./locales";
import * as client from "./lib/client";

import App from "./App.vue";
import "./index.css";

import router from "./router";
import { createI18n } from "vue-i18n";

client.GetCurrentLanguage().then((lan) => {
  const i18n = createI18n({
    locale: lan || "en",
    fallbackLocale: "en", // set fallback locale
    messages: languages,
  });

  const app = createApp(App);

  app.use(createPinia());
  app.use(router);
  app.use(i18n);
  app.mount("#app");
});
