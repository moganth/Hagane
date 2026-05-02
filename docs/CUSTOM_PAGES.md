# Custom Pages

Custom pages let an installer ask the user for extra information that does not fit the built-in welcome, license, install directory, component, summary, or finish screens.

In this codebase, a custom page is still a first-class installer page. It participates in normal navigation, validation, state snapshots, and variable substitution. The main difference is that you define the page data in the manifest instead of hard-coding the UI in Rust.

## What A Custom Page Does

A custom page can:

- Collect text, multiline text, checkbox, choice, and folder path values.
- Bind those values to installer variables such as `CERT_DIR` or `IMPORT_CERTS`.
- Block `Next` until required fields are valid.
- Pass the collected values into later install steps and hooks.
- Optionally render advanced raw HTML when you need more control than the built-in widgets provide.

## When To Use It

Use a custom page when you need one or more of these:

- A folder selection that is specific to your product.
- A setup question that changes later install behavior.
- A small decision form before the install starts.
- A page that needs several fields bound to variables.

Do not use a custom page when a built-in page already solves the problem. For example, the install directory page should still be used for the main target folder.

## How The Runtime Uses It

The flow is simple:

1. The manifest declares a page with `type: custom`.
2. The engine parses the page and validates its widget definitions.
3. The runner loads the generic `custom.html` template for that page.
4. The template renders widgets or raw custom HTML.
5. User input is sent back to Rust through IPC.
6. Rust stores the values in installer state.
7. When the install starts, the runner merges those custom values into the variable map.
8. Later install steps and hooks can reference those values with `{{VARIABLE_NAME}}`.

That means custom page values behave like normal installer variables once the install begins.

## Manifest Shape

A custom page is defined under `pages`.

A minimal page looks like this:

```yaml
pages:
  - type: custom
    id: cert_folder
    title: "Certificate Folder"
    subtitle: "Choose where certificate files should be read from during setup."
    widgets:
      - type: folder_picker
        id: cert_dir
        label: "Certificates folder"
        bind_to: CERT_DIR
        default: "{{INSTDIR}}/certs"
        browse_title: "Select certificate folder"
        help_text: "This path is exposed to install steps as {{CERT_DIR}}."
        required: true
        must_exist: false
```

The important fields are:

- `type: custom` identifies the page as a custom page.
- `id` gives the page a stable identifier.
- `title` is the page heading.
- `subtitle` is optional explanatory text.
- `widgets` defines the controls that the user sees.

You can also use `custom_html` for advanced rendering, but the widget-based flow is the recommended path.

## Widget Reference

The supported widget types are deliberately small and predictable.

### `label`

A read-only text block. Use it for instructions, warnings, or separators.

Example:

```yaml
- type: label
  id: cert_hint
  text: "This folder will be used for certificate import during setup."
```

### `text_input`

A single-line text field. Bind it to a variable if you want the value available later.

Useful fields:

- `label`
- `bind_to`
- `default`
- `placeholder`
- `required`
- `min_length`
- `max_length`

Example:

```yaml
- type: text_input
  id: company_name
  label: "Company name"
  bind_to: COMPANY_NAME
  required: true
```

### `multiline_input`

A larger text box for notes, commands, or free-form content.

Example:

```yaml
- type: multiline_input
  id: install_notes
  label: "Notes"
  bind_to: INSTALL_NOTES
  placeholder: "Optional notes for the installer"
```

### `checkbox`

A boolean toggle.

Example:

```yaml
- type: checkbox
  id: import_certs
  label: "Import certificates during setup"
  bind_to: IMPORT_CERTS
  default: true
```

### `radio_group`

A set of mutually exclusive choices where the user must pick one option.

Useful fields:

- `label`
- `bind_to`
- `required`
- `options`

Example:

```yaml
- type: radio_group
  id: mode
  label: "Install mode"
  bind_to: INSTALL_MODE
  required: true
  options:
    - label: "Standard"
      value: standard
    - label: "Advanced"
      value: advanced
```

### `dropdown`

A compact choice selector.

Example:

```yaml
- type: dropdown
  id: region
  label: "Region"
  bind_to: REGION
  options:
    - label: "US"
      value: us
    - label: "EU"
      value: eu
```

### `folder_picker`

A field that opens the native folder browser. This is the best option when the user needs to point the installer at an existing directory.

Useful fields:

- `label`
- `bind_to`
- `default`
- `browse_title`
- `help_text`
- `required`
- `must_exist`

Example:

```yaml
- type: folder_picker
  id: cert_dir
  label: "Certificates folder"
  bind_to: CERT_DIR
  default: "{{INSTDIR}}/certs"
  browse_title: "Select certificate folder"
  required: true
```

## Binding Values To Later Steps

The `bind_to` field is what turns a widget value into an installer variable.

If a widget is bound to `CERT_DIR`, later steps can use that variable like this:

```yaml
hooks:
  post_install:
    - run:
        shell: powershell
        wait: true
        fail_on_nonzero: true
        timeout_sec: 30
        command: |
          Write-Host "Certificate folder: {{CERT_DIR}}"
```

That is the key idea behind custom pages: the UI collects data once, then the rest of the installer uses it like any other variable.

## Validation Rules

The validator checks several things before the installer is built:

- Custom pages must have a non-empty `id`.
- Page ids must not be duplicated.
- A custom page must define either `custom_html` or at least one widget.
- Interactive widgets must have a non-empty `bind_to` value.
- Choice widgets must contain options.
- Widget ids must be unique within the page.

This is intentional. Validation fails early so the manifest is not allowed to ship with broken page wiring.

## Example Walkthrough

The example manifest at [sdk/example/installer.yaml](../sdk/example/installer.yaml) shows a concrete page that asks for a certificate folder and a boolean import flag.

The flow is:

1. The page appears after component selection.
2. The user chooses a folder in the native folder picker.
3. The user optionally toggles whether certificates should be imported.
4. The installer stores those values as `CERT_DIR` and `IMPORT_CERTS`.
5. The post-install hook prints the chosen values.

That is the simplest real-world pattern to start with.

## Raw HTML Mode

If `custom_html` is present, the generic template renders that HTML instead of generating widgets.

Use this mode only when you need something the widget set does not support yet. It is powerful, but it is also easier to make mistakes with because the installer cannot infer validation from arbitrary HTML.

When using raw HTML, you are responsible for sending values back to the engine through the page script.

## Practical Recommendations

- Prefer widgets over `custom_html` for most pages.
- Keep the page focused on a small number of decisions.
- Bind every user-entered value that later install logic depends on.
- Use clear page ids and variable names so the manifest stays readable.
- Test the page in a full build, not only by reading the YAML.

If you follow those rules, custom pages stay predictable and easy to maintain.
