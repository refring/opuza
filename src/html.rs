use {
  crate::common::*,
  maud::{html, DOCTYPE},
};

pub(crate) fn wrap_body(title_slug: &str, body: Markup) -> Response<Body> {
  let html = html! {
    (DOCTYPE)
    html lang="en" {
      head {
        meta charset="utf-8";
        meta name="viewport" content="width=device-width, initial-scale=1";
        title {
          (format!("{} · Opuza", title_slug))
        }
        link rel="stylesheet" href="/static/index.css";
        script type="module" src="/static/index.js" {}
      }
      body {
        main {
          (body)
        }
        footer {
          "Powered by "
          a href="https://github.com/refring/opuza" {
            "Opuza"
          }
          "."
        }
      }
    }
  };
  Response::builder()
    .header(header::CONTENT_TYPE, "text/html")
    .body(Body::from(html.into_string()))
    .expect("builder arguments are valid")
}
