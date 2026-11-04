//! Code snippets for the right panel (mockup `.rb-lang` menu), generated
//! from the same input as the cURL export so every language describes the
//! exact request Send would make. Values are emitted as written, so
//! `{{variables}}` stay visible as placeholders.

use base64::Engine as _;

use crate::services::curl::{GenerateCurlInput, generate as generate_curl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    CSharpHttpClient,
    #[default]
    Curl,
    GoNative,
    Http,
    JavaOkHttp,
    JsFetch,
    NodeAxios,
    PythonRequests,
    RubyNetHttp,
    ShellWget,
}

impl Lang {
    /// Menu order: alphabetical by label, as in the mockup.
    pub const ALL: [Lang; 10] = [
        Lang::CSharpHttpClient,
        Lang::Curl,
        Lang::GoNative,
        Lang::Http,
        Lang::JavaOkHttp,
        Lang::JsFetch,
        Lang::NodeAxios,
        Lang::PythonRequests,
        Lang::RubyNetHttp,
        Lang::ShellWget,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Lang::CSharpHttpClient => "C# - HttpClient",
            Lang::Curl => "cURL",
            Lang::GoNative => "Go - Native",
            Lang::Http => "HTTP",
            Lang::JavaOkHttp => "Java - OkHttp",
            Lang::JsFetch => "JavaScript - Fetch",
            Lang::NodeAxios => "Node - Axios",
            Lang::PythonRequests => "Python - requests",
            Lang::RubyNetHttp => "Ruby - Net::HTTP",
            Lang::ShellWget => "Shell - wget",
        }
    }

    /// Stable key for the session (labels may be reworded).
    pub fn key(self) -> &'static str {
        match self {
            Lang::CSharpHttpClient => "csharp-httpclient",
            Lang::Curl => "curl",
            Lang::GoNative => "go-native",
            Lang::Http => "http",
            Lang::JavaOkHttp => "java-okhttp",
            Lang::JsFetch => "js-fetch",
            Lang::NodeAxios => "node-axios",
            Lang::PythonRequests => "python-requests",
            Lang::RubyNetHttp => "ruby-nethttp",
            Lang::ShellWget => "shell-wget",
        }
    }

    pub fn from_key(key: &str) -> Lang {
        Lang::ALL
            .into_iter()
            .find(|l| l.key() == key)
            .unwrap_or_default()
    }

    /// Which strings the panel colours: `'…'` for shell, `"…"` elsewhere.
    pub fn quote(self) -> Option<char> {
        match self {
            Lang::Curl | Lang::ShellWget => Some('\''),
            Lang::Http => None,
            _ => Some('"'),
        }
    }
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The request reduced to what every language needs.
struct Req {
    method: String,
    url: String,
    /// Includes auth and cookies, in send order.
    headers: Vec<(String, String)>,
    body: Body,
}

enum Body {
    None,
    Raw(String),
    /// `application/x-www-form-urlencoded`.
    Form(Vec<(String, String)>),
    /// `multipart/form-data`: (key, value, is_file). File values are names.
    Multipart(Vec<(String, String, bool)>),
}

impl Req {
    fn content_type(&self) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
    }
}

fn normalize(input: &GenerateCurlInput) -> Req {
    let method = match input.method.trim() {
        "" => "GET".to_owned(),
        m => m.to_uppercase(),
    };
    let mut url = input.url.trim().to_owned();
    let mut headers = Vec::new();
    match input.auth_type.as_str() {
        "bearer" => {
            if let Some(t) = input.bearer_token.as_deref().filter(|t| !t.is_empty()) {
                headers.push(("Authorization".to_owned(), format!("Bearer {t}")));
            }
        }
        "basic" => {
            if let Some(u) = input.basic_user.as_deref().filter(|u| !u.is_empty()) {
                let p = input.basic_pass.as_deref().unwrap_or("");
                let enc = base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"));
                headers.push(("Authorization".to_owned(), format!("Basic {enc}")));
            }
        }
        "apikey" => {
            if let (Some(n), Some(v)) = (
                input.api_key_name.as_deref().filter(|n| !n.is_empty()),
                input.api_key_value.as_deref().filter(|v| !v.is_empty()),
            ) {
                if input.api_key_location.as_deref() == Some("query") {
                    let sep = if url.contains('?') { '&' } else { '?' };
                    url = format!("{url}{sep}{n}={v}");
                } else {
                    headers.push((n.to_owned(), v.to_owned()));
                }
            }
        }
        _ => {}
    }
    for kv in input.headers.iter().filter(|kv| !kv.key.is_empty()) {
        headers.push((kv.key.clone(), kv.value.clone()));
    }
    let cookies: Vec<String> = input
        .cookies
        .iter()
        .filter(|kv| !kv.key.is_empty())
        .map(|kv| format!("{}={}", kv.key, kv.value))
        .collect();
    if !cookies.is_empty() {
        headers.push(("Cookie".to_owned(), cookies.join("; ")));
    }
    let fields: Vec<_> = input
        .form_fields
        .iter()
        .filter(|f| !f.key.is_empty())
        .collect();
    let body = if !fields.is_empty() {
        if fields.iter().any(|f| f.is_file) {
            Body::Multipart(
                fields
                    .iter()
                    .map(|f| {
                        let v = if f.is_file {
                            f.file_name.clone().unwrap_or_else(|| "file".to_owned())
                        } else {
                            f.value.clone()
                        };
                        (f.key.clone(), v, f.is_file)
                    })
                    .collect(),
            )
        } else {
            Body::Form(
                fields
                    .iter()
                    .map(|f| (f.key.clone(), f.value.clone()))
                    .collect(),
            )
        }
    } else {
        match input.body.as_deref() {
            Some(b) if !b.is_empty() => Body::Raw(b.to_owned()),
            _ => Body::None,
        }
    };
    Req {
        method,
        url,
        headers,
        body,
    }
}

pub fn generate(lang: Lang, input: &GenerateCurlInput) -> String {
    if lang == Lang::Curl {
        return generate_curl(input);
    }
    let r = normalize(input);
    match lang {
        Lang::Curl => unreachable!(),
        Lang::Http => http(&r),
        Lang::JsFetch => js_fetch(&r),
        Lang::NodeAxios => node_axios(&r),
        Lang::PythonRequests => python(&r),
        Lang::GoNative => go(&r),
        Lang::JavaOkHttp => java(&r),
        Lang::CSharpHttpClient => csharp(&r),
        Lang::RubyNetHttp => ruby(&r),
        Lang::ShellWget => wget(&r),
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// A double-quoted literal with JSON escapes — valid in JS, Python, Go,
/// Java and C# (all accept `\"`, `\\`, `\n`, `\uXXXX`).
fn dq(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned())
}

/// Ruby double quotes also interpolate `#{…}`.
fn rb(s: &str) -> String {
    dq(s).replace("#{", "\\#{")
}

/// POSIX single-quoted word.
fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn form_string(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// (host, path-with-query) for the HTTP view. `{{base_url}}/x` keeps the
/// variable as the host, as Postman does.
fn split_url(url: &str) -> (String, String) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    if url.contains("://") || rest.starts_with("{{") {
        let cut = if rest.starts_with("{{") {
            rest.find("}}").map(|i| i + 2).unwrap_or(rest.len())
        } else {
            rest.find(['/', '?']).unwrap_or(rest.len())
        };
        let (host, path) = rest.split_at(cut);
        let path = if path.is_empty() || path.starts_with('?') {
            format!("/{path}")
        } else {
            path.to_owned()
        };
        (host.to_owned(), path)
    } else {
        (String::new(), url.to_owned())
    }
}

// ── languages ────────────────────────────────────────────────────────────────

fn http(r: &Req) -> String {
    let (host, path) = split_url(&r.url);
    let mut out = format!("{} {} HTTP/1.1\n", r.method, path);
    if !host.is_empty() {
        out.push_str(&format!("Host: {host}\n"));
    }
    for (k, v) in &r.headers {
        out.push_str(&format!("{k}: {v}\n"));
    }
    match &r.body {
        Body::None => {}
        Body::Raw(b) => out.push_str(&format!("Content-Length: {}\n\n{b}", b.len())),
        Body::Form(p) => {
            let b = form_string(p);
            if r.content_type().is_none() {
                out.push_str("Content-Type: application/x-www-form-urlencoded\n");
            }
            out.push_str(&format!("Content-Length: {}\n\n{b}", b.len()));
        }
        Body::Multipart(p) => {
            let boundary = "----RustmanBoundary";
            out.push_str(&format!(
                "Content-Type: multipart/form-data; boundary={boundary}\n\n"
            ));
            for (k, v, file) in p {
                out.push_str(&format!("--{boundary}\n"));
                if *file {
                    out.push_str(&format!(
                        "Content-Disposition: form-data; name=\"{k}\"; filename=\"{v}\"\n\n(file contents)\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "Content-Disposition: form-data; name=\"{k}\"\n\n{v}\n"
                    ));
                }
            }
            out.push_str(&format!("--{boundary}--"));
        }
    }
    out.trim_end().to_owned()
}

fn js_fetch(r: &Req) -> String {
    let mut out = String::new();
    let has_headers = !r.headers.is_empty();
    if has_headers {
        out.push_str("const myHeaders = new Headers();\n");
        for (k, v) in &r.headers {
            out.push_str(&format!("myHeaders.append({}, {});\n", dq(k), dq(v)));
        }
        out.push('\n');
    }
    let body = match &r.body {
        Body::None => None,
        Body::Raw(b) => {
            out.push_str(&format!("const raw = {};\n\n", dq(b)));
            Some("raw")
        }
        Body::Form(p) => {
            out.push_str("const urlencoded = new URLSearchParams();\n");
            for (k, v) in p {
                out.push_str(&format!("urlencoded.append({}, {});\n", dq(k), dq(v)));
            }
            out.push('\n');
            Some("urlencoded")
        }
        Body::Multipart(p) => {
            out.push_str("const formdata = new FormData();\n");
            for (k, v, file) in p {
                if *file {
                    out.push_str(&format!(
                        "formdata.append({}, fileInput.files[0], {});\n",
                        dq(k),
                        dq(v)
                    ));
                } else {
                    out.push_str(&format!("formdata.append({}, {});\n", dq(k), dq(v)));
                }
            }
            out.push('\n');
            Some("formdata")
        }
    };
    out.push_str("const requestOptions = {\n");
    out.push_str(&format!("  method: {},\n", dq(&r.method)));
    if has_headers {
        out.push_str("  headers: myHeaders,\n");
    }
    if let Some(b) = body {
        out.push_str(&format!("  body: {b},\n"));
    }
    out.push_str("  redirect: \"follow\"\n};\n\n");
    out.push_str(&format!("fetch({}, requestOptions)\n", dq(&r.url)));
    out.push_str("  .then((response) => response.text())\n");
    out.push_str("  .then((result) => console.log(result))\n");
    out.push_str("  .catch((error) => console.error(error));");
    out
}

fn node_axios(r: &Req) -> String {
    let mut out = String::from("const axios = require(\"axios\");\n");
    let data = match &r.body {
        Body::None => None,
        Body::Raw(b) => {
            out.push_str(&format!("let data = {};\n", dq(b)));
            Some(())
        }
        Body::Form(p) => {
            out.push_str("const qs = require(\"qs\");\nlet data = qs.stringify({\n");
            for (k, v) in p {
                out.push_str(&format!("  {}: {},\n", dq(k), dq(v)));
            }
            out.push_str("});\n");
            Some(())
        }
        Body::Multipart(p) => {
            out.push_str(
                "const FormData = require(\"form-data\");\nconst fs = require(\"fs\");\nlet data = new FormData();\n",
            );
            for (k, v, file) in p {
                if *file {
                    out.push_str(&format!(
                        "data.append({}, fs.createReadStream({}));\n",
                        dq(k),
                        dq(v)
                    ));
                } else {
                    out.push_str(&format!("data.append({}, {});\n", dq(k), dq(v)));
                }
            }
            Some(())
        }
    };
    out.push_str("\nlet config = {\n");
    out.push_str(&format!("  method: {},\n", dq(&r.method.to_lowercase())));
    out.push_str("  maxBodyLength: Infinity,\n");
    out.push_str(&format!("  url: {},\n", dq(&r.url)));
    if !r.headers.is_empty() || matches!(r.body, Body::Multipart(_)) {
        out.push_str("  headers: {\n");
        for (k, v) in &r.headers {
            out.push_str(&format!("    {}: {},\n", dq(k), dq(v)));
        }
        if matches!(r.body, Body::Multipart(_)) {
            out.push_str("    ...data.getHeaders(),\n");
        }
        out.push_str("  },\n");
    }
    if data.is_some() {
        out.push_str("  data: data,\n");
    }
    out.push_str("};\n\n");
    out.push_str("axios.request(config)\n");
    out.push_str(
        "  .then((response) => {\n    console.log(JSON.stringify(response.data));\n  })\n",
    );
    out.push_str("  .catch((error) => {\n    console.log(error);\n  });");
    out
}

fn python(r: &Req) -> String {
    let mut out = String::from("import requests\n\n");
    out.push_str(&format!("url = {}\n\n", dq(&r.url)));
    let mut files = false;
    match &r.body {
        Body::None => out.push_str("payload = {}\n"),
        Body::Raw(b) => out.push_str(&format!("payload = {}\n", dq(b))),
        Body::Form(p) => {
            out.push_str("payload = {\n");
            for (k, v) in p {
                out.push_str(&format!("    {}: {},\n", dq(k), dq(v)));
            }
            out.push_str("}\n");
        }
        Body::Multipart(p) => {
            out.push_str("payload = {\n");
            for (k, v, file) in p {
                if !file {
                    out.push_str(&format!("    {}: {},\n", dq(k), dq(v)));
                }
            }
            out.push_str("}\nfiles = [\n");
            for (k, v, file) in p {
                if *file {
                    out.push_str(&format!(
                        "    ({}, ({}, open({}, \"rb\"))),\n",
                        dq(k),
                        dq(v),
                        dq(v)
                    ));
                }
            }
            out.push_str("]\n");
            files = true;
        }
    }
    if r.headers.is_empty() {
        out.push_str("headers = {}\n\n");
    } else {
        out.push_str("headers = {\n");
        for (k, v) in &r.headers {
            out.push_str(&format!("    {}: {},\n", dq(k), dq(v)));
        }
        out.push_str("}\n\n");
    }
    out.push_str(&format!(
        "response = requests.request({}, url, headers=headers, data=payload{})\n\n",
        dq(&r.method),
        if files { ", files=files" } else { "" }
    ));
    out.push_str("print(response.text)");
    out
}

fn go(r: &Req) -> String {
    let multipart = matches!(r.body, Body::Multipart(_));
    let mut imports = vec!["\"fmt\"", "\"net/http\"", "\"io\""];
    match &r.body {
        Body::Raw(_) | Body::Form(_) => imports.insert(1, "\"strings\""),
        Body::Multipart(_) => {
            imports.insert(1, "\"bytes\"");
            imports.insert(2, "\"mime/multipart\"");
        }
        Body::None => {}
    }
    let mut out = String::from("package main\n\nimport (\n");
    for i in &imports {
        out.push_str(&format!("  {i}\n"));
    }
    out.push_str(")\n\nfunc main() {\n\n");
    out.push_str(&format!(
        "  url := {}\n  method := {}\n\n",
        dq(&r.url),
        dq(&r.method)
    ));
    let payload = match &r.body {
        Body::None => "nil",
        Body::Raw(b) => {
            out.push_str(&format!("  payload := strings.NewReader({})\n\n", dq(b)));
            "payload"
        }
        Body::Form(p) => {
            out.push_str(&format!(
                "  payload := strings.NewReader({})\n\n",
                dq(&form_string(p))
            ));
            "payload"
        }
        Body::Multipart(p) => {
            out.push_str(
                "  payload := &bytes.Buffer{}\n  writer := multipart.NewWriter(payload)\n",
            );
            for (k, v, file) in p {
                if *file {
                    out.push_str(&format!("  // attach file {} as field {}\n", dq(v), dq(k)));
                } else {
                    out.push_str(&format!("  _ = writer.WriteField({}, {})\n", dq(k), dq(v)));
                }
            }
            out.push_str("  if err := writer.Close(); err != nil {\n    fmt.Println(err)\n    return\n  }\n\n");
            "payload"
        }
    };
    out.push_str("  client := &http.Client{}\n");
    out.push_str(&format!(
        "  req, err := http.NewRequest(method, url, {payload})\n\n"
    ));
    out.push_str("  if err != nil {\n    fmt.Println(err)\n    return\n  }\n");
    for (k, v) in &r.headers {
        out.push_str(&format!("  req.Header.Add({}, {})\n", dq(k), dq(v)));
    }
    if multipart {
        out.push_str("  req.Header.Set(\"Content-Type\", writer.FormDataContentType())\n");
    }
    out.push_str("\n  res, err := client.Do(req)\n  if err != nil {\n    fmt.Println(err)\n    return\n  }\n");
    out.push_str("  defer res.Body.Close()\n\n");
    out.push_str("  body, err := io.ReadAll(res.Body)\n  if err != nil {\n    fmt.Println(err)\n    return\n  }\n");
    out.push_str("  fmt.Println(string(body))\n}");
    out
}

fn java(r: &Req) -> String {
    let mut out =
        String::from("OkHttpClient client = new OkHttpClient().newBuilder()\n  .build();\n");
    let body = match &r.body {
        Body::None => "null",
        Body::Raw(b) => {
            let ct = r.content_type().unwrap_or("text/plain");
            out.push_str(&format!(
                "MediaType mediaType = MediaType.parse({});\n",
                dq(ct)
            ));
            out.push_str(&format!(
                "RequestBody body = RequestBody.create(mediaType, {});\n",
                dq(b)
            ));
            "body"
        }
        Body::Form(p) => {
            out.push_str(
                "MediaType mediaType = MediaType.parse(\"application/x-www-form-urlencoded\");\n",
            );
            out.push_str(&format!(
                "RequestBody body = RequestBody.create(mediaType, {});\n",
                dq(&form_string(p))
            ));
            "body"
        }
        Body::Multipart(p) => {
            out.push_str(
                "RequestBody body = new MultipartBody.Builder().setType(MultipartBody.FORM)\n",
            );
            for (k, v, file) in p {
                if *file {
                    out.push_str(&format!(
                        "  .addFormDataPart({}, {},\n    RequestBody.create(MediaType.parse(\"application/octet-stream\"),\n    new File({})))\n",
                        dq(k),
                        dq(v),
                        dq(v)
                    ));
                } else {
                    out.push_str(&format!("  .addFormDataPart({}, {})\n", dq(k), dq(v)));
                }
            }
            out.push_str("  .build();\n");
            "body"
        }
    };
    out.push_str("Request request = new Request.Builder()\n");
    out.push_str(&format!("  .url({})\n", dq(&r.url)));
    out.push_str(&format!("  .method({}, {body})\n", dq(&r.method)));
    for (k, v) in &r.headers {
        out.push_str(&format!("  .addHeader({}, {})\n", dq(k), dq(v)));
    }
    out.push_str("  .build();\nResponse response = client.newCall(request).execute();");
    out
}

fn csharp(r: &Req) -> String {
    let mut out = String::from("var client = new HttpClient();\n");
    out.push_str(&format!(
        "var request = new HttpRequestMessage(new HttpMethod({}), {});\n",
        dq(&r.method),
        dq(&r.url)
    ));
    // Content-Type belongs to the content, not the request headers (adding
    // it there throws in .NET).
    for (k, v) in r
        .headers
        .iter()
        .filter(|(k, _)| !k.eq_ignore_ascii_case("content-type"))
    {
        out.push_str(&format!("request.Headers.Add({}, {});\n", dq(k), dq(v)));
    }
    match &r.body {
        Body::None => {}
        Body::Raw(b) => {
            let ct = r.content_type().unwrap_or("text/plain");
            out.push_str(&format!(
                "var content = new StringContent({}, null, {});\nrequest.Content = content;\n",
                dq(b),
                dq(ct)
            ));
        }
        Body::Form(p) => {
            out.push_str("var collection = new List<KeyValuePair<string, string>>();\n");
            for (k, v) in p {
                out.push_str(&format!("collection.Add(new({}, {}));\n", dq(k), dq(v)));
            }
            out.push_str("var content = new FormUrlEncodedContent(collection);\nrequest.Content = content;\n");
        }
        Body::Multipart(p) => {
            out.push_str("var content = new MultipartFormDataContent();\n");
            for (k, v, file) in p {
                if *file {
                    out.push_str(&format!(
                        "content.Add(new StreamContent(File.OpenRead({})), {}, {});\n",
                        dq(v),
                        dq(k),
                        dq(v)
                    ));
                } else {
                    out.push_str(&format!(
                        "content.Add(new StringContent({}), {});\n",
                        dq(v),
                        dq(k)
                    ));
                }
            }
            out.push_str("request.Content = content;\n");
        }
    }
    out.push_str("var response = await client.SendAsync(request);\n");
    out.push_str("response.EnsureSuccessStatusCode();\n");
    out.push_str("Console.WriteLine(await response.Content.ReadAsStringAsync());");
    out
}

fn ruby(r: &Req) -> String {
    let class = {
        let m = r.method.to_lowercase();
        let mut c = m.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => "Get".to_owned(),
        }
    };
    let mut out = String::from("require \"uri\"\nrequire \"net/http\"\n\n");
    out.push_str(&format!("url = URI({})\n\n", rb(&r.url)));
    out.push_str("http = Net::HTTP.new(url.host, url.port)\n");
    if r.url.trim_start().to_lowercase().starts_with("https") {
        out.push_str("http.use_ssl = true\n");
    }
    out.push_str(&format!("\nrequest = Net::HTTP::{class}.new(url)\n"));
    for (k, v) in &r.headers {
        out.push_str(&format!("request[{}] = {}\n", rb(k), rb(v)));
    }
    match &r.body {
        Body::None => {}
        Body::Raw(b) => out.push_str(&format!("request.body = {}\n", rb(b))),
        Body::Form(p) => {
            let pairs: Vec<String> = p
                .iter()
                .map(|(k, v)| format!("{} => {}", rb(k), rb(v)))
                .collect();
            out.push_str(&format!(
                "request.set_form_data({{{}}})\n",
                pairs.join(", ")
            ));
        }
        Body::Multipart(p) => {
            let parts: Vec<String> = p
                .iter()
                .map(|(k, v, file)| {
                    if *file {
                        format!("[{}, File.open({})]", rb(k), rb(v))
                    } else {
                        format!("[{}, {}]", rb(k), rb(v))
                    }
                })
                .collect();
            out.push_str(&format!(
                "form_data = [{}]\nrequest.set_form form_data, \"multipart/form-data\"\n",
                parts.join(", ")
            ));
        }
    }
    out.push_str("\nresponse = http.request(request)\nputs response.read_body");
    out
}

fn wget(r: &Req) -> String {
    let mut parts = vec![
        "wget --quiet".to_owned(),
        format!("--method {}", r.method),
        "--timeout=0".to_owned(),
    ];
    for (k, v) in &r.headers {
        parts.push(format!("--header {}", sq(&format!("{k}: {v}"))));
    }
    match &r.body {
        Body::None => {}
        Body::Raw(b) => parts.push(format!("--body-data {}", sq(b))),
        Body::Form(p) => parts.push(format!("--body-data {}", sq(&form_string(p)))),
        // wget has no multipart builder; send the text fields urlencoded and
        // say so rather than emit something that silently drops files.
        Body::Multipart(p) => {
            let text: Vec<(String, String)> = p
                .iter()
                .filter(|(_, _, f)| !f)
                .map(|(k, v, _)| (k.clone(), v.clone()))
                .collect();
            parts.push(format!("--body-data {}", sq(&form_string(&text))));
            parts.push("# note: wget cannot upload files; file fields omitted".to_owned());
        }
    }
    parts.push(format!("-O - {}", sq(&r.url)));
    parts.join(" \\\n  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::curl::{FormFieldInput, KvPair};

    fn input() -> GenerateCurlInput {
        GenerateCurlInput {
            method: "POST".into(),
            url: "{{base_url}}/posts".into(),
            headers: vec![KvPair {
                key: "Content-Type".into(),
                value: "application/json".into(),
            }],
            body: Some("{\"title\": \"Hi\"}".into()),
            form_fields: vec![],
            cookies: vec![],
            auth_type: "bearer".into(),
            bearer_token: Some("{{TOKEN}}".into()),
            basic_user: None,
            basic_pass: None,
            api_key_name: None,
            api_key_value: None,
            api_key_location: None,
        }
    }

    #[test]
    fn every_language_carries_method_url_headers_and_body() {
        let i = input();
        for lang in Lang::ALL {
            let s = generate(lang, &i);
            assert!(s.contains("{{base_url}}"), "{lang}: url\n{s}");
            assert!(s.contains("{{TOKEN}}"), "{lang}: auth\n{s}");
            assert!(s.contains("application/json"), "{lang}: header\n{s}");
            assert!(s.contains("title"), "{lang}: body\n{s}");
            assert!(s.to_uppercase().contains("POST"), "{lang}: method\n{s}");
        }
    }

    #[test]
    fn http_splits_variable_host_and_path() {
        assert_eq!(
            split_url("{{base_url}}/v1/login"),
            ("{{base_url}}".into(), "/v1/login".into())
        );
        assert_eq!(
            split_url("https://api.test?q=1"),
            ("api.test".into(), "/?q=1".into())
        );
    }

    #[test]
    fn ruby_strings_cannot_interpolate() {
        assert_eq!(rb("#{x}"), "\"\\#{x}\"");
    }

    #[test]
    fn urlencoded_form_and_multipart_are_distinguished() {
        let mut i = input();
        i.body = None;
        i.form_fields = vec![FormFieldInput {
            key: "name".into(),
            value: "a b".into(),
            is_file: false,
            file_name: None,
        }];
        assert!(generate(Lang::PythonRequests, &i).contains("\"name\": \"a b\""));
        assert!(generate(Lang::Http, &i).contains("name=a+b"));
        i.form_fields.push(FormFieldInput {
            key: "doc".into(),
            value: String::new(),
            is_file: true,
            file_name: Some("x.pdf".into()),
        });
        assert!(generate(Lang::PythonRequests, &i).contains("files=files"));
        assert!(generate(Lang::JsFetch, &i).contains("new FormData()"));
    }

    #[test]
    fn keys_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_key(lang.key()), lang);
        }
        assert_eq!(Lang::from_key("nope"), Lang::Curl);
    }
}
