import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import CompanionView from './components/CompanionView.vue'
import './style.css'

const view = new URLSearchParams(location.search).get('view')
createApp(view === 'mini' || view === 'desktop-lyrics' ? CompanionView : App).use(createPinia()).mount('#app')
