// PostCSS pipeline for Tailwind. ESM form (package.json is "type": "module").
import tailwindcss from "tailwindcss";
import autoprefixer from "autoprefixer";

export default {
  plugins: [tailwindcss, autoprefixer],
};
