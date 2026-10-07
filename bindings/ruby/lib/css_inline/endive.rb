# frozen_string_literal: true

require "json"

module CSSInline
  module Endive
    RESULT_SIZE = 8
    WASM_PATH = File.expand_path("../../target/wasm32-wasip1/release/css_inline_wasm.wasm", __dir__)

    java_import "run.endive.runtime.ImportValues"
    java_import "run.endive.runtime.Instance"
    java_import "run.endive.wasi.WasiOptions"
    java_import "run.endive.wasi.WasiPreview1"
    java_import "run.endive.wasm.Parser"

    begin
      java_import "org.cssinline.wasm.CssInlineModule"
    rescue NameError
      # The source-build interpreter remains available when no compiled machine is packaged.
    end

    class Runtime
      def initialize
        @wasi = WasiPreview1.builder.with_options(WasiOptions.builder.build).build
        imports = ImportValues.builder.with_functions(@wasi.to_host_functions.to_a).build
        compiled_module = Endive.compiled_module
        builder = Instance.builder(compiled_module ? compiled_module.wasm_module : Endive.wasm_module)
        builder.with_machine_factory(compiled_module.machine_factory) if compiled_module
        @instance = builder.with_import_values(imports).build
        @memory = @instance.memory
        @mutex = Mutex.new
      end

      def inline(html, options)
        call("css_inline_inline", html, options)
      end

      def inline_fragment(html, css, options)
        call("css_inline_inline_fragment", html, css, options)
      end

      def validate_options(options)
        call("css_inline_validate_options", options)
      end

      private

      def call(name, *arguments)
        @mutex.synchronize do
          result_pointer = allocate(RESULT_SIZE)
          strings = arguments.map { |argument| write_string(argument) }
          called = false

          function(name).apply(result_pointer, *strings.map(&:first))
          called = true

          unwrap(result_pointer)
        ensure
          function("css_inline_result_free").apply(result_pointer) if called
          deallocate(result_pointer, RESULT_SIZE) if result_pointer
          strings&.each { |pointer, size| deallocate(pointer, size) }
        end
      end

      def function(name)
        @instance.export(name)
      end

      def allocate(size)
        function("css_inline_allocate").apply(size)[0].to_i
      end

      def deallocate(pointer, size)
        function("css_inline_deallocate").apply(pointer, size)
      end

      def write_string(value)
        raise ArgumentError, "string contains null byte" if value.include?("\0")

        bytes = "#{value}\0".b.to_java_bytes
        pointer = allocate(bytes.length)
        @memory.write(pointer, bytes)

        [pointer, bytes.length]
      end

      def unwrap(result_pointer)
        value_pointer = @memory.read_int(result_pointer)
        error_pointer = @memory.read_int(result_pointer + 4)

        raise ArgumentError, @memory.readCString(error_pointer) if error_pointer.nonzero?
        raise ArgumentError, "css-inline returned no result" if value_pointer.zero?

        @memory.readCString(value_pointer)
      end
    end

    module_function

    def inline(html, options)
      runtime.inline(html, options)
    end

    def inline_fragment(html, css, options)
      runtime.inline_fragment(html, css, options)
    end

    def validate_options(options)
      runtime.validate_options(options)
    end

    def runtime
      @runtime ||= Runtime.new
    end

    def compiled?
      !compiled_module.nil?
    end

    def compiled_module
      return unless const_defined?(:CssInlineModule, false)

      @compiled_module ||= CssInlineModule.new
    end

    def wasm_module
      @wasm_module ||= begin
        raise LoadError, "css-inline WebAssembly module not found at #{WASM_PATH}" unless File.file?(WASM_PATH)

        Parser.parse(java.io.File.new(WASM_PATH))
      end
    end
  end

  class StylesheetCache
    def initialize(size: 8)
      raise ArgumentError, "Cache size must be an integer greater than zero" unless size.is_a?(Integer) && size.positive?

      @size = size
    end
  end

  class CSSInliner
    def initialize(**options)
      @options = JSON.generate(options.reject { |key| key == :cache })
      Endive.validate_options(@options)
    end

    def inline(html)
      Endive.inline(String(html), @options)
    end

    def inline_fragment(html, css)
      Endive.inline_fragment(String(html), String(css), @options)
    end

    def inline_many(documents)
      documents.map { |html| inline(html) }
    end

    def inline_many_fragments(documents, stylesheets)
      documents.first(stylesheets.length).zip(stylesheets).map { |html, css| inline_fragment(html, css) }
    end
  end

  module_function

  def inline(html, **options)
    CSSInliner.new(**options).inline(html)
  end

  def inline_fragment(html, css, **options)
    CSSInliner.new(**options).inline_fragment(html, css)
  end

  def inline_many(documents, **options)
    CSSInliner.new(**options).inline_many(documents)
  end

  def inline_many_fragments(documents, stylesheets, **options)
    CSSInliner.new(**options).inline_many_fragments(documents, stylesheets)
  end
end
