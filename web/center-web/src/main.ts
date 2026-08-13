import { createApp } from "vue";
import { createPinia } from "pinia";
import Antd from "ant-design-vue";
import "ant-design-vue/dist/antd.css";
import App from "./App.vue";
import "./style.css";

createApp(App).use(createPinia()).use(Antd).mount("#app");
