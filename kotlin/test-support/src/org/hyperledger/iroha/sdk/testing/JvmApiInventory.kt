package org.hyperledger.iroha.sdk.testing

import java.io.DataInputStream

/** Inspect compiled API contracts without reflective access or SDK runtime dependencies. */
internal class JvmApiInventory private constructor(
    val superName: String,
    val fields: List<Member>,
    val methods: List<Member>,
) {
    class Member(val flags: Int, val name: String, val descriptor: String, val parameterNames: List<String>) {
        val isPublic: Boolean get() = flags and 0x0001 != 0
        val isStatic: Boolean get() = flags and 0x0008 != 0
        val isNative: Boolean get() = flags and 0x0100 != 0
        val isSynthetic: Boolean get() = flags and 0x1000 != 0
        val parameterTypes: List<String> get() {
            require(descriptor.startsWith("("))
            val types = mutableListOf<String>()
            var offset = 1
            while (descriptor[offset] != ')') {
                val start = offset
                while (descriptor[offset] == '[') offset++
                offset = if (descriptor[offset] == 'L') descriptor.indexOf(';', offset) + 1 else offset + 1
                require(offset > start)
                types.add(descriptor.substring(start, offset))
            }
            return types
        }
    }

    companion object {
        fun read(type: Class<*>): JvmApiInventory {
            val path = "/" + type.name.replace('.', '/') + ".class"
            val stream = checkNotNull(type.getResourceAsStream(path)) { "missing compiled class $path" }
            return DataInputStream(stream).use { input ->
                check(input.readInt() == 0xcafebabe.toInt())
                input.readUnsignedShort() // minor version
                input.readUnsignedShort() // major version
                val utf8 = arrayOfNulls<String>(input.readUnsignedShort())
                val classes = IntArray(utf8.size)
                var index = 1
                while (index < utf8.size) {
                    when (val tag = input.readUnsignedByte()) {
                        1 -> utf8[index] = input.readUTF()
                        7 -> classes[index] = input.readUnsignedShort()
                        3, 4, 9, 10, 11, 12, 17, 18 -> input.readInt()
                        5, 6 -> { input.readLong(); index++ }
                        8, 16, 19, 20 -> input.readUnsignedShort()
                        15 -> { input.readUnsignedByte(); input.readUnsignedShort() }
                        else -> error("unknown classfile constant tag $tag")
                    }
                    index++
                }
                fun text(index: Int): String = checkNotNull(utf8[index])
                input.readUnsignedShort() // class flags
                input.readUnsignedShort() // this class
                val parent = text(classes[input.readUnsignedShort()])
                repeat(input.readUnsignedShort()) { input.readUnsignedShort() }
                fun members(): List<Member> = List(input.readUnsignedShort()) {
                    val flags = input.readUnsignedShort()
                    val name = text(input.readUnsignedShort())
                    val descriptor = text(input.readUnsignedShort())
                    val names = mutableListOf<String>()
                    repeat(input.readUnsignedShort()) {
                        val attribute = text(input.readUnsignedShort())
                        val bytes = ByteArray(input.readInt())
                        input.readFully(bytes)
                        if (attribute == "MethodParameters") {
                            DataInputStream(bytes.inputStream()).use { parameters ->
                                repeat(parameters.readUnsignedByte()) {
                                    val nameIndex = parameters.readUnsignedShort()
                                    if (nameIndex != 0) names.add(text(nameIndex))
                                    parameters.readUnsignedShort()
                                }
                                check(parameters.available() == 0)
                            }
                        }
                    }
                    Member(flags, name, descriptor, names)
                }
                JvmApiInventory(parent, members(), members())
            }
        }
    }
}
